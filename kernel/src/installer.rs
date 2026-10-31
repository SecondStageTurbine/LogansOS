// SPDX-License-Identifier: MPL-2.0
//! OT-87 R4.3: the installer (`docs/THINK-2026-10-27-ot87-installer.md` §4).
//!
//! It writes one claimed disk (R4.1's [`ClaimedWriter`], the only path past
//! the write guard) from bytes already in RAM, so the boot medium is never
//! read while it runs:
//!
//! - the **bundle** (`/install/bundle`, R4.2's `NEOINST1`), held to the BLAKE3
//!   pin xtask compiled in (`LOGANSOS_INSTALL_BUNDLE_BLAKE3`) and then part by
//!   part to its manifest: the ESP template and slot map for the target's
//!   sector size, and the NeoFS seed;
//! - the **running system** for slot A and `/deploy/A/`: the kernel file
//!   Limine loaded (its executable-file response) and the four userland
//!   modules (§4.3 item 3);
//! - the **pinned model and vocab** modules, streamed to NeoFS and held to
//!   their pins through NeoFS's verified content hash (§4.3 item 4).
//!
//! [`preflight`] makes every refusal before the first write (§4.6, §4.7 rule
//! 1, a slot file too small for its source). [`install`] then runs
//! `install_logic`'s W1–W6 with this module's [`Payload`] for W3–W5:
//!
//! - **W3**: the ESP template over partition 1, except inside the slot files'
//!   extents; then slot A's files and their directory entries' `FileSize`.
//! - **W4**: the NeoFS seed over the start of partition 2, then the volume
//!   grown to the whole partition.
//! - **W5**: the model and vocab under `/neofs/`, the deployment copy under
//!   `/deploy/A/`, synced. `limine.conf` already names slot A in the
//!   template, so W5 doesn't rewrite it.
//!
//! Process context, interrupts on: every call issues block I/O and may park.

extern crate alloc;

use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, Ordering};

use crate::boot_model_logic;
use crate::boot_modules::{self, ModuleLease};
use crate::drivers::block::claim_logic::{self, ClaimKind, Enumerated};
use crate::drivers::block::install_claim::{self, ClaimedWriter};
use crate::drivers::block::install_logic::{
    self as logic, Class, Failure, Guids, Outcome, Payload, Start, Step, Tables, Target,
};
use crate::drivers::block::partition::PartitionDevice;
use crate::drivers::block::{
    BlockDevice, BlockDeviceIdentity, BlockError, BlockGeometry, BlockOp, BlockRequest,
    BlockResult, DmaBuffer, gpt_logic,
};
use crate::fs::neofs::NeoFs;
use crate::fs::{Inode, VfsError};
use crate::install_bundle_logic as bundle;
use crate::limine_file_logic;
use crate::serial_println;
use crate::sync::SwitchMutex;

/// Where the Install entry loads the bundle (§4.3, §6.1).
pub const BUNDLE_MODULE: &str = "/install/bundle";
/// The bundle's BLAKE3, compiled in by xtask (§4.3 item 2).
pub const PIN: Option<&str> = option_env!("LOGANSOS_INSTALL_BUNDLE_BLAKE3");
/// The userland modules a deployment carries, as slot A and `/deploy/A/`
/// name them; the kernel is the first slot file.
pub const USERLAND: [&str; 4] = ["neo-init", "neo-worker", "neo-shell", "neo-arthur"];
/// The largest request the installer builds (the claim splits it further).
const CHUNK: usize = 1024 * 1024;

/// Why an install didn't start.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Refused {
    NoBundle,
    NoPin,
    PinMismatch,
    Bundle(bundle::BundleError),
    NoKernelFile,
    MissingModule(&'static str),
    NoModel,
    /// A deployment file is larger than its slot file (§7.2's sizes).
    SlotTooSmall {
        file: &'static str,
        bytes: u64,
        extent: u64,
    },
    Claim(claim_logic::Refusal),
    Target(logic::Refusal),
}

/// The bundle, checked against the pin and its manifest.
pub struct Bundle {
    bytes: &'static [u8],
    parts: Vec<bundle::Part>,
}

impl Bundle {
    fn part(&self, name: &str) -> &'static [u8] {
        let part = self
            .parts
            .iter()
            .find(|p| p.name == name)
            .expect("installer_manifest checked the names");
        // The manifest and every part's hash were checked in `load`.
        &self.bytes[part.offset as usize..(part.offset + part.length) as usize]
    }

    fn expanded(&self, name: &str) -> u64 {
        self.parts
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.expanded)
            .unwrap_or(0)
    }

    /// The template, its runs and its slot map for sector size `s`.
    fn esp(&self, s: u64) -> Result<(Vec<bundle::Run<'static>>, bundle::SlotMap), Refused> {
        let (esp, slots) = if s == 512 {
            (bundle::ESP_512, bundle::SLOTS_512)
        } else {
            (bundle::ESP_4096, bundle::SLOTS_4096)
        };
        let runs =
            bundle::sparse_runs(self.part(esp), self.expanded(esp)).map_err(Refused::Bundle)?;
        let map = bundle::slot_map(self.part(slots)).map_err(Refused::Bundle)?;
        if map.sector_size as u64 != s {
            return Err(Refused::Bundle(bundle::BundleError::BadSectorSize));
        }
        Ok((runs, map))
    }
}

/// Load the bundle module and hold it to the pin, then each part to the
/// manifest.
pub fn load_bundle() -> Result<Bundle, Refused> {
    let bytes = boot_modules::find(BUNDLE_MODULE).ok_or(Refused::NoBundle)?;
    let pin = PIN.ok_or(Refused::NoPin)?;
    let digest = boot_model_logic::hex_digest(blake3::hash(bytes).as_bytes());
    let digest = core::str::from_utf8(&digest).unwrap_or("");
    serial_println!(
        "[ kernel ] install: bundle bytes={} blake3={} pin={} match={}",
        bytes.len(),
        digest,
        pin,
        digest.eq_ignore_ascii_case(pin)
    );
    if !digest.eq_ignore_ascii_case(pin) {
        return Err(Refused::PinMismatch);
    }
    let parts = bundle::installer_manifest(bytes).map_err(Refused::Bundle)?;
    for part in &parts {
        bundle::check_part(bytes, part).map_err(Refused::Bundle)?;
    }
    Ok(Bundle { bytes, parts })
}

/// The kernel file as Limine loaded it. SAFETY (for the `'static`): Limine
/// keeps the executable file mapped for the kernel's life; nothing frees it.
fn kernel_file() -> Option<&'static limine::file::File> {
    crate::EXECUTABLE_FILE_REQUEST
        .response()
        .map(|r| r.executable_file())
}

fn kernel_bytes() -> Option<&'static [u8]> {
    let data = kernel_file()?.data();
    // SAFETY: see `kernel_file`.
    Some(unsafe { core::slice::from_raw_parts(data.as_ptr(), data.len()) })
}

/// The boot medium's GPT disk GUID, as GPT stores it (Limine's file
/// response; zero when the medium has no GPT). Read at the protocol's offset,
/// not through the `limine` crate's field (`limine_file_logic`).
pub fn boot_disk_guid() -> Option<[u8; 16]> {
    let file = kernel_file()? as *const limine::file::File as *const u8;
    // SAFETY: Limine's response points at a whole `struct limine_file`
    // (`limine_file_logic::FILE_BYTES`, every revision), mapped for the
    // kernel's life (see `kernel_file`); read unaligned as plain bytes.
    let raw =
        unsafe { core::ptr::read_unaligned(file as *const [u8; limine_file_logic::FILE_BYTES]) };
    limine_file_logic::gpt_disk_guid(&raw)
}

/// The running system's deployment: (slot file name, bytes), kernel first.
fn deployment() -> Result<Vec<(&'static str, &'static [u8])>, Refused> {
    let mut out = vec![("kernel", kernel_bytes().ok_or(Refused::NoKernelFile)?)];
    for name in USERLAND {
        let suffix = match name {
            "neo-init" => "/neo-init",
            "neo-worker" => "/neo-worker",
            "neo-shell" => "/neo-shell",
            _ => "/neo-arthur",
        };
        out.push((
            name,
            boot_modules::find(suffix).ok_or(Refused::MissingModule(name))?,
        ));
    }
    Ok(out)
}

/// A pinned model or vocab and its bytes, for W5.
struct Pinned {
    /// `/neofs/<name>`'s name.
    name: &'static str,
    /// Every digest pinned for its module (test kernels pin two for one
    /// path); W5's read-back must match one of them.
    pins: Vec<[u8; 32]>,
    bytes: &'static [u8],
}

/// Add `module`'s pin to its entry in `out`, or a new entry with `bytes`.
fn add_pin(
    out: &mut Vec<Pinned>,
    module: &'static str,
    blake3: [u8; 32],
    bytes: &'static [u8],
) -> Result<(), Refused> {
    let name = boot_model_logic::neofs_name(module).ok_or(Refused::NoModel)?;
    match out.iter_mut().find(|p| p.name == name) {
        Some(p) => p.pins.push(blake3),
        None => out.push(Pinned {
            name,
            pins: vec![blake3],
            bytes,
        }),
    }
    Ok(())
}

/// The consumable model modules, taken once per boot and held for the whole
/// install session: a Finish installing in the same boot needs them again.
/// They are reclaimed with the session, which R5's install mode ends (THINK
/// §4.3 item 4).
static MODEL_LEASES: SwitchMutex<Vec<(&'static str, ModuleLease)>> = SwitchMutex::new(Vec::new());

/// Every pin whose module is present: models taken (once), vocabs found.
fn pinned_sources() -> Result<Vec<Pinned>, Refused> {
    let mut out = Vec::new();
    let mut leases = MODEL_LEASES.lock();
    for pin in boot_modules::PINS {
        if !leases.iter().any(|(m, _)| *m == pin.module)
            && let Some(lease) = boot_modules::take(pin.module)
        {
            leases.push((pin.module, lease));
        }
        if let Some((_, lease)) = leases.iter().find(|(m, _)| *m == pin.module) {
            let bytes = lease.bytes();
            // SAFETY: the lease is held in a static for the rest of the boot
            // and never reclaimed while it is (see `MODEL_LEASES`).
            let bytes: &'static [u8] =
                unsafe { core::slice::from_raw_parts(bytes.as_ptr(), bytes.len()) };
            add_pin(&mut out, pin.module, pin.blake3, bytes)?;
        }
    }
    if out.is_empty() {
        return Err(Refused::NoModel);
    }
    for pin in boot_modules::VOCAB_PINS {
        if let Some(bytes) = boot_modules::find(pin.module) {
            add_pin(&mut out, pin.module, pin.blake3, bytes)?;
        }
    }
    Ok(out)
}

/// The claim, shared: the state machine writes through it, and so does the
/// NeoFS mount in W4 and W5 (through [`ClaimDevice`]).
type Shared = Arc<SwitchMutex<ClaimedWriter>>;

/// The claimed disk as `install_logic`'s target.
pub struct ClaimTarget {
    claim: Shared,
    sector: u64,
}

impl Target for ClaimTarget {
    fn sector_size(&self) -> u64 {
        self.sector
    }

    fn write(&mut self, lba: u64, bytes: &[u8]) -> bool {
        self.claim.lock().write(lba * self.sector, bytes).is_ok()
    }

    fn flush(&mut self) -> bool {
        self.claim.lock().flush().is_ok()
    }

    fn read(&mut self, lba: u64, out: &mut [u8]) -> bool {
        self.claim.lock().read(lba * self.sector, out).is_ok()
    }

    fn boundary(&mut self) -> bool {
        self.claim.lock().boundary().is_ok()
    }
}

/// The claimed disk read as a GPT disk, for [`logic::classify`] and the
/// partition view.
struct ClaimDisk<'a> {
    claim: &'a Shared,
    capacity: u64,
    sector: u64,
}

impl gpt_logic::Disk for ClaimDisk<'_> {
    fn capacity(&self) -> u64 {
        self.capacity
    }

    fn sector_size(&self) -> u64 {
        self.sector
    }

    fn read(&self, offset: u64, out: &mut [u8]) -> bool {
        // Whole sectors through the claim; `out` may be any span.
        let first = offset / self.sector;
        let end = (offset + out.len() as u64).div_ceil(self.sector);
        let mut buf = vec![0u8; ((end - first) * self.sector) as usize];
        if self
            .claim
            .lock()
            .read(first * self.sector, &mut buf)
            .is_err()
        {
            return false;
        }
        let skip = (offset - first * self.sector) as usize;
        out.copy_from_slice(&buf[skip..skip + out.len()]);
        true
    }
}

/// The claimed disk as a block device, so NeoFS (W4, W5) mounts partition 2
/// through the claim, with the claim's checks on every request.
pub(crate) struct ClaimDevice {
    claim: Shared,
    identity: BlockDeviceIdentity,
    geometry: BlockGeometry,
}

impl ClaimDevice {
    /// The whole claimed disk: `neofs_grow_write` mounts a bare NeoFS volume
    /// through it, to write past the volume's old end as W4/W5 do.
    #[cfg(feature = "neo-test-neofs-grow-write")]
    pub(crate) fn whole(writer: ClaimedWriter) -> Self {
        let identity = writer.identity();
        let geometry = writer.geometry();
        Self {
            claim: Arc::new(SwitchMutex::new(writer)),
            identity,
            geometry,
        }
    }
}

impl BlockDevice for ClaimDevice {
    fn identity(&self) -> &BlockDeviceIdentity {
        &self.identity
    }

    fn geometry(&self) -> BlockGeometry {
        self.geometry
    }

    fn submit(&self, request: BlockRequest<'_>) -> BlockResult<()> {
        match request.op {
            BlockOp::Read => self.read_block(request.lba, request.buffers),
            BlockOp::Write => self.write_block(request.lba, request.buffers),
            BlockOp::Flush => self.flush(),
        }
    }

    fn supports_flush(&self) -> bool {
        true
    }

    fn flush(&self) -> BlockResult<()> {
        self.claim.lock().flush().map_err(|_| BlockError::Io)
    }

    fn read_block(&self, lba: u64, buffers: &[Arc<DmaBuffer>]) -> BlockResult<()> {
        let mut offset = lba * self.geometry.logical_block_size as u64;
        let mut claim = self.claim.lock();
        for buffer in buffers {
            // SAFETY: the caller owns `buffer` for this synchronous request.
            let out = unsafe { buffer.as_mut_slice() };
            claim.read(offset, out).map_err(|_| BlockError::Io)?;
            offset += out.len() as u64;
        }
        Ok(())
    }

    fn write_block(&self, lba: u64, buffers: &[Arc<DmaBuffer>]) -> BlockResult<()> {
        let mut offset = lba * self.geometry.logical_block_size as u64;
        let mut claim = self.claim.lock();
        for buffer in buffers {
            // SAFETY: the caller owns `buffer` for this synchronous request.
            let bytes = unsafe { buffer.as_slice() };
            claim.write(offset, bytes).map_err(|_| BlockError::Io)?;
            offset += bytes.len() as u64;
        }
        Ok(())
    }
}

/// Everything checked: the target admitted, its claim held, the bytes ready.
pub struct Prepared {
    claim: Shared,
    sector: u64,
    capacity: u64,
    tables: Tables,
    start: Start,
    bundle: Bundle,
    deployment: Vec<(&'static str, &'static [u8])>,
    pinned: Vec<Pinned>,
}

impl Prepared {
    pub fn start(&self) -> Start {
        self.start
    }

    pub fn tables(&self) -> &Tables {
        &self.tables
    }

    pub fn slot(&self) -> usize {
        self.claim.lock().slot()
    }
}

static GUID_COUNTER: AtomicU64 = AtomicU64::new(0);

/// `install_refusals`' no-flush row: no driver lacks flush, so the scenario
/// makes the next preflights see a device without it (§4.7 rule 1).
#[cfg(feature = "neo-test-install")]
static TEST_NO_FLUSH: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

#[cfg(feature = "neo-test-install")]
pub fn test_set_no_flush(on: bool) {
    TEST_NO_FLUSH.store(on, Ordering::Relaxed);
}

fn target_flushes(writer: &ClaimedWriter) -> bool {
    #[cfg(feature = "neo-test-install")]
    if TEST_NO_FLUSH.load(Ordering::Relaxed) {
        return false;
    }
    writer.supports_flush()
}

/// A fresh v4 GUID, as GPT stores it: BLAKE3 over the TSC, the target's
/// identity and a counter. Unique per install, which is all a GPT GUID needs
/// (the kernel's DRBG is a separate feature).
fn fresh_guid(seed: &[u8]) -> [u8; 16] {
    let mut h = blake3::Hasher::new();
    h.update(b"LogansOS install GUID");
    // SAFETY: RDTSC has no preconditions.
    h.update(&unsafe { core::arch::x86_64::_rdtsc() }.to_le_bytes());
    h.update(&GUID_COUNTER.fetch_add(1, Ordering::Relaxed).to_le_bytes());
    h.update(seed);
    let mut g = [0u8; 16];
    g.copy_from_slice(&h.finalize().as_bytes()[..16]);
    // Version 4 (the top nibble of the third field, stored little-endian at
    // bytes 6..8) and the RFC 4122 variant (byte 8).
    g[7] = (g[7] & 0x0f) | 0x40;
    g[8] = (g[8] & 0x3f) | 0x80;
    g
}

/// Every refusal, before any write (W0): the bundle and its pin, the
/// deployment and the model, the claim (R4.1's refusals), the target's
/// sector size, size and flush, its class against `start`, and every slot
/// file against its source. `start` is [`Start::Fresh`] for a blank disk and
/// [`Start::Finish`] for an incomplete one.
pub fn preflight(row: &Enumerated, start: Start) -> Result<Prepared, Refused> {
    let bundle = load_bundle()?;
    let deployment = deployment()?;
    let pinned = pinned_sources()?;
    let writer =
        install_claim::claim(row, ClaimKind::Fresh, boot_disk_guid()).map_err(Refused::Claim)?;
    let geometry = writer.geometry();
    let sector = geometry.logical_block_size as u64;
    let capacity = geometry.block_count * sector;
    let flush = target_flushes(&writer);
    let claim: Shared = Arc::new(SwitchMutex::new(writer));
    logic::check_target(capacity, sector, flush).map_err(Refused::Target)?;
    let class = logic::classify(&ClaimDisk {
        claim: &claim,
        capacity,
        sector,
    });
    let guids = match logic::admit(class, start).map_err(Refused::Target)? {
        Some(guids) => guids,
        None => {
            let id = claim.lock().identity();
            let seed = [id.driver.as_bytes(), &capacity.to_le_bytes()].concat();
            Guids {
                disk: fresh_guid(&seed),
                esp: fresh_guid(&seed),
                neofs: fresh_guid(&seed),
            }
        }
    };
    let tables = Tables::new(sector, capacity / sector - 1, guids)
        .ok_or(Refused::Target(logic::Refusal::NoLayout))?;
    let (_, map) = bundle.esp(sector)?;
    for (name, bytes) in &deployment {
        let path = slot_a(name);
        let extent = map
            .entries
            .iter()
            .find(|e| e.path == path)
            .map(|e| e.sectors * sector)
            .unwrap_or(0);
        if bytes.len() as u64 > extent {
            return Err(Refused::SlotTooSmall {
                file: name,
                bytes: bytes.len() as u64,
                extent,
            });
        }
    }
    serial_println!(
        "[ kernel ] install: preflight slot={} S={} L={} class={:?} start={:?} esp={:?} neofs={:?} neofs_blocks={}",
        claim.lock().slot(),
        sector,
        capacity / sector - 1,
        class_name(class),
        start,
        tables.complete.esp,
        tables.complete.neofs,
        tables.complete.neofs_total_blocks
    );
    Ok(Prepared {
        claim,
        sector,
        capacity,
        tables,
        start,
        bundle,
        deployment,
        pinned,
    })
}

fn class_name(class: Class) -> &'static str {
    match class {
        Class::Blank => "blank",
        Class::Incomplete(_) => "incomplete",
        Class::Installed(_) => "installed",
        Class::Foreign => "foreign",
    }
}

fn slot_a(name: &str) -> String {
    let mut p = String::from("slot/A/");
    p.push_str(name);
    p
}

/// Run W1–W6 (or W3–W6 to finish), stopping after `stop_after` if given.
/// The claim is released when this returns.
pub fn install(prepared: Prepared, stop_after: Option<Step>) -> Result<Outcome, Failure> {
    let mut target = ClaimTarget {
        claim: prepared.claim.clone(),
        sector: prepared.sector,
    };
    let mut payload = InstallPayload { p: &prepared };
    let tables = prepared.tables.clone();
    let result = logic::install(
        &mut target,
        &tables,
        &mut payload,
        prepared.start,
        stop_after,
    );
    match result {
        Ok(outcome) => {
            serial_println!("[ kernel ] install: done outcome={:?}", outcome);
        }
        Err(failure) => {
            serial_println!(
                "[ kernel ] install: stopped step={} failure={:?}",
                failure.step().name(),
                failure
            );
        }
    }
    result
}

/// Where a step's planned writes go: submitted (`write`) or read back and
/// compared (`verify`), so both walk the same plan.
type Sink<'a> = dyn FnMut(u64, &[u8]) -> Result<(), Failure> + 'a;

/// One file W5 writes under `/neofs/`.
struct W5File {
    path: String,
    bytes: &'static [u8],
    /// The digests its read-back may match.
    digests: Vec<[u8; 32]>,
}

/// W3–W5 for one prepared install.
struct InstallPayload<'a> {
    p: &'a Prepared,
}

impl InstallPayload<'_> {
    fn s(&self) -> u64 {
        self.p.sector
    }

    /// Every W3 write, in order, handed to `f` as (disk LBA, bytes).
    fn w3(&self, f: &mut Sink<'_>) -> Result<(), Failure> {
        let s = self.s();
        let fail = Failure::Write {
            step: Step::W3,
            lba: 0,
        };
        let (runs, map) = self.p.bundle.esp(s).map_err(|_| fail)?;
        let esp_first = self.p.tables.complete.esp.0;
        let esp_bytes = (self.p.tables.complete.esp.1 + 1 - esp_first) * s;
        // The template, except inside the slot files' extents (slot A gets
        // its files next; slot B stays as it is until OT-51 writes it).
        let mut skip: Vec<(u64, u64)> = map
            .entries
            .iter()
            .filter(|e| e.path.starts_with("slot/"))
            .map(|e| (e.first_sector * s, (e.first_sector + e.sectors) * s))
            .collect();
        skip.sort();
        // Slot A's directory entries carry their files' sizes: patched into
        // the template as it is expanded, so every sector has one planned
        // value (the read-back compares against the same bytes).
        let mut sizes: Vec<(u64, u32)> = Vec::new();
        for (name, bytes) in &self.p.deployment {
            let path = slot_a(name);
            let entry = map.entries.iter().find(|e| e.path == path).ok_or(fail)?;
            sizes.push((
                entry.dirent_sector * s + entry.dirent_offset as u64 + 28,
                bytes.len() as u32,
            ));
        }
        let expand = |from: u64, out: &mut [u8]| {
            bundle::expand_into(&runs, from, out);
            let end = from + out.len() as u64;
            for &(at, size) in &sizes {
                // A FileSize field is 4 bytes inside one directory sector,
                // so it never straddles a write.
                if at >= from && at + 4 <= end {
                    let i = (at - from) as usize;
                    out[i..i + 4].copy_from_slice(&size.to_le_bytes());
                }
            }
        };
        let mut buf = vec![0u8; CHUNK];
        let mut at = 0u64;
        while at < esp_bytes {
            let end = (at + CHUNK as u64).min(esp_bytes);
            let mut from = at;
            for &(a, b) in skip.iter().filter(|(a, b)| *b > at && *a < end) {
                if a > from {
                    let out = &mut buf[..(a - from) as usize];
                    expand(from, out);
                    f(esp_first + from / s, out)?;
                }
                from = from.max(b);
            }
            if from < end {
                let out = &mut buf[..(end - from) as usize];
                expand(from, out);
                f(esp_first + from / s, out)?;
            }
            at = end;
        }
        // Slot A's files, whole sectors, the tail zero-padded.
        for (name, bytes) in &self.p.deployment {
            let path = slot_a(name);
            let entry = map.entries.iter().find(|e| e.path == path).ok_or(fail)?;
            let mut done = 0usize;
            while done < bytes.len() {
                let take = (bytes.len() - done).min(CHUNK);
                let padded = take.div_ceil(s as usize) * s as usize;
                buf[..take].copy_from_slice(&bytes[done..done + take]);
                buf[take..padded].fill(0);
                f(
                    esp_first + entry.first_sector + done as u64 / s,
                    &buf[..padded],
                )?;
                done += take;
            }
        }
        Ok(())
    }

    /// The seed's bytes over the start of partition 2, (disk LBA, bytes).
    fn w4_seed(&self, f: &mut Sink<'_>) -> Result<(), Failure> {
        let s = self.s();
        let fail = Failure::Write {
            step: Step::W4,
            lba: 0,
        };
        let seed_len = self.p.bundle.expanded(bundle::NEOFS_SEED);
        let runs = bundle::sparse_runs(self.p.bundle.part(bundle::NEOFS_SEED), seed_len)
            .map_err(|_| fail)?;
        let first = self.p.tables.complete.neofs.0;
        let mut buf = vec![0u8; CHUNK];
        let mut at = 0u64;
        while at < seed_len {
            let end = (at + CHUNK as u64).min(seed_len);
            let out = &mut buf[..(end - at) as usize];
            bundle::expand_into(&runs, at, out);
            f(first + at / s, out)?;
            at = end;
        }
        Ok(())
    }

    /// Partition 2 as a block device over the claim.
    fn neofs_device(&self, step: Step) -> Result<Arc<dyn BlockDevice>, Failure> {
        let claim = &self.p.claim;
        let disk = ClaimDisk {
            claim,
            capacity: self.p.capacity,
            sector: self.p.sector,
        };
        let fail = Failure::Verify { step, lba: 1 };
        let gpt = gpt_logic::read(&disk).ok_or(fail)?;
        let entry = gpt
            .entries
            .iter()
            .find(|e| e.index == 2)
            .copied()
            .ok_or(fail)?;
        let (identity, geometry) = {
            let c = claim.lock();
            (c.identity(), c.geometry())
        };
        let whole: Arc<dyn BlockDevice> = Arc::new(ClaimDevice {
            claim: claim.clone(),
            identity,
            geometry,
        });
        let part = PartitionDevice::new(whole, &gpt, &entry).ok_or(fail)?;
        Ok(Arc::new(part))
    }

    fn w5_files(&self) -> Vec<W5File> {
        let mut files = Vec::new();
        for pin in &self.p.pinned {
            files.push(W5File {
                path: String::from(pin.name),
                bytes: pin.bytes,
                digests: pin.pins.clone(),
            });
        }
        for (name, bytes) in &self.p.deployment {
            let mut path = String::from("deploy/A/");
            path.push_str(name);
            files.push(W5File {
                path,
                bytes,
                digests: vec![*blake3::hash(bytes).as_bytes()],
            });
        }
        files
    }
}

/// [`logic::verify`], saying on a failure whether the read failed or where
/// the bytes first differ.
fn verify_logged(
    target: &mut ClaimTarget,
    step: Step,
    lba: u64,
    expected: &[u8],
) -> Result<(), Failure> {
    let mut back = vec![0u8; expected.len()];
    let read = target.read(lba, &mut back);
    if read && back == expected {
        return Ok(());
    }
    let first = back.iter().zip(expected).position(|(a, b)| a != b);
    serial_println!(
        "[ kernel ] install: step={} verify lba={} bytes={} read_ok={} first_diff={:?} got={:02x?} want={:02x?}",
        step.name(),
        lba,
        expected.len(),
        read,
        first,
        first.map(|i| &back[i..(i + 16).min(back.len())]),
        first.map(|i| &expected[i..(i + 16).min(expected.len())]),
    );
    Err(Failure::Verify { step, lba })
}

fn neofs_failed(step: Step, what: &str, e: VfsError) -> Failure {
    serial_println!(
        "[ kernel ] install: step={} neofs {} err={:?}",
        step.name(),
        what,
        e
    );
    Failure::Write { step, lba: 0 }
}

impl Payload<ClaimTarget> for InstallPayload<'_> {
    fn write(&mut self, step: Step, target: &mut ClaimTarget) -> Result<(), Failure> {
        match step {
            Step::W3 => self.w3(&mut |lba, bytes| {
                if target.write(lba, bytes) {
                    Ok(())
                } else {
                    Err(Failure::Write { step, lba })
                }
            }),
            Step::W4 => {
                self.w4_seed(&mut |lba, bytes| {
                    if target.write(lba, bytes) {
                        Ok(())
                    } else {
                        Err(Failure::Write { step, lba })
                    }
                })?;
                if !target.flush() {
                    return Err(Failure::Flush { step });
                }
                let fs = NeoFs::mount_readwrite(self.neofs_device(step)?).map_err(|e| {
                    serial_println!("[ kernel ] install: step=W4 seed mount err={:?}", e);
                    Failure::Verify { step, lba: 0 }
                })?;
                let total = self.p.tables.complete.neofs_total_blocks;
                fs.grow(total).map_err(|e| neofs_failed(step, "grow", e))?;
                fs.sync().map_err(|e| neofs_failed(step, "sync", e))?;
                serial_println!(
                    "[ kernel ] install: step=W4 neofs grown total_blocks={}",
                    total
                );
                Ok(())
            }
            Step::W5 => {
                let fs = NeoFs::mount_readwrite(self.neofs_device(step)?).map_err(|e| {
                    serial_println!("[ kernel ] install: step=W5 mount err={:?}", e);
                    Failure::Verify { step, lba: 0 }
                })?;
                for dir in ["deploy", "deploy/A"] {
                    fs.create_directory(dir)
                        .map_err(|e| neofs_failed(step, "mkdir", e))?;
                }
                // Each file in one piece (NFS-W1), through the claim.
                for W5File { path, bytes, .. } in self.w5_files() {
                    fs.create_file_from_bytes(&path, bytes)
                        .map_err(|e| neofs_failed(step, "create", e))?;
                    serial_println!(
                        "[ kernel ] install: step=W5 wrote /neofs/{} bytes={}",
                        path,
                        bytes.len()
                    );
                }
                fs.sync().map_err(|e| neofs_failed(step, "sync", e))?;
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn verify(&mut self, step: Step, target: &mut ClaimTarget) -> Result<(), Failure> {
        match step {
            Step::W3 => self.w3(&mut |lba, bytes| verify_logged(target, step, lba, bytes)),
            Step::W4 => {
                let fs = NeoFs::mount_readonly(self.neofs_device(step)?).map_err(|e| {
                    serial_println!("[ kernel ] install: step=W4 verify mount err={:?}", e);
                    Failure::Verify { step, lba: 0 }
                })?;
                if fs.total_blocks() != self.p.tables.complete.neofs_total_blocks {
                    return Err(Failure::Verify { step, lba: 0 });
                }
                Ok(())
            }
            Step::W5 => {
                let fs = NeoFs::mount_readonly(self.neofs_device(step)?).map_err(|e| {
                    serial_println!("[ kernel ] install: step=W5 verify mount err={:?}", e);
                    Failure::Verify { step, lba: 0 }
                })?;
                for W5File {
                    path,
                    bytes,
                    digests: expected,
                } in self.w5_files()
                {
                    let inode: Arc<dyn Inode> = fs
                        .lookup(&path)
                        .map_err(|_| Failure::Verify { step, lba: 0 })?;
                    let verified = inode.verified_content_hash().ok().flatten();
                    let ok = inode.byte_len() == Some(bytes.len() as u64)
                        && verified.is_some_and(|v| expected.contains(&v.blake3));
                    serial_println!(
                        "[ kernel ] install: step=W5 verify /neofs/{} bytes={} content_hash_matches={}",
                        path,
                        bytes.len(),
                        ok
                    );
                    if !ok {
                        return Err(Failure::Verify { step, lba: 0 });
                    }
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}
