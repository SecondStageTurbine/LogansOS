// SPDX-License-Identifier: MPL-2.0
//! B1a: the boot-module registry (`Plans/DECISION-2026-10-19-arthur-first-boot-model.md`
//! Amendment 1 (c), Chad C2).
//!
//! Before B1a, every Limine module lived for the kernel's lifetime and any
//! caller could mint a `'static` slice of it at any time. Under
//! `model-boot-module` that contract becomes:
//!
//! - **Ordinary modules** (`neo-init`, the user ELFs, firmware) still live
//!   forever and are found by [`find`], exactly as before.
//! - **Consumable modules** (a path containing [`CONSUMABLE_DIR`]) are never
//!   handed out as a `'static` slice. They can be [`take`]n once, as a
//!   [`ModuleLease`] whose bytes borrow from the lease. After the take, every
//!   lookup of that path returns `None`: the entry is tombstoned for the life
//!   of the kernel.
//! - [`ModuleLease::reclaim`] consumes the lease (so no borrow of its bytes can
//!   survive it), plans the module's whole pages against the memmap, the
//!   executable and every other module (`mm::module_reclaim_logic`), and
//!   admits them to the frame allocator as a new zone. A refused plan admits
//!   nothing and the pages stay retained (fail closed).
//!
//! B1b adds the two consumers:
//!
//! - [`load_model`]: `sys_model_load` of a `/boot/models/<file>` path takes the
//!   pinned module, hashes every byte, builds the model only on a match, and
//!   then reclaims the module's pages.
//! - [`mount_boot_fs`]: a read-only `/boot` that serves the *ordinary* modules
//!   (Arthur's vocab), so a system with no NeoFS disk can still read them.
//!   B1c: a module under `/vocab/` is served only if pinned, and only after
//!   its bytes matched the pin (hashed once, the verdict cached).

// The scenario-only probes (`debug_forged_plan`) are dead in product builds.
#![cfg_attr(not(feature = "neo-test-boot-module-reclaim"), allow(dead_code))]

use alloc::sync::Arc;
use core::sync::atomic::{AtomicU8, Ordering};

use crate::boot_model_logic::{self, ModelPin, Verdict};
use crate::errno::{EBADMSG, ENOENT, EPERM};
use crate::fs::{FileRights, Inode, InodeKind, MountedFs, VfsError};
use crate::mm::module_reclaim_logic::{MapEntry, PlanRefusal, Span, plan_module_reclaim};
use crate::mm::{AdmitError, PhysExtent};
use crate::serial_println;

/// Paths containing this are consumable (staged by `xtask iso --with-model`).
pub const CONSUMABLE_DIR: &str = "/models/";

const PRESENT: u8 = 0;
const TAKEN: u8 = 1;

static STATE: [AtomicU8; crate::MAX_BOOT_MODULE_EXTENTS] =
    [const { AtomicU8::new(PRESENT) }; crate::MAX_BOOT_MODULE_EXTENTS];

/// Upper bound on memmap entries a reclaim plan reads (real machines report
/// tens; more is refused rather than truncated).
const MAX_MEMMAP_ENTRIES: usize = 256;

fn modules() -> &'static [&'static limine::file::File] {
    crate::MODULE_REQUEST
        .response()
        .map(|r| r.modules())
        .unwrap_or(&[])
}

fn is_consumable(path: &str) -> bool {
    path.contains(CONSUMABLE_DIR)
}

/// The module's bytes. SAFETY (for the `'static`): Limine module data is
/// HHDM-mapped and never freed by Limine; it is only ever handed to the
/// allocator by [`ModuleLease::reclaim`], which consumes the sole lease.
fn module_bytes(module: &limine::file::File) -> &'static [u8] {
    let slice = module.data();
    // SAFETY: see the function doc.
    unsafe { core::slice::from_raw_parts(slice.as_ptr(), slice.len()) }
}

/// B1c: the ordinary module `/boot` serves for `suffix`. A `/vocab/` module
/// must be pinned and its bytes must match the pin; the first lookup hashes
/// it, later lookups reuse the verdict. Two racing first lookups both hash
/// and store the same verdict.
fn find_verified(suffix: &str) -> Result<&'static [u8], VfsError> {
    let (index, module) = modules()
        .iter()
        .enumerate()
        .find(|(_, m)| m.path().ends_with(suffix))
        .ok_or(VfsError::NotFound)?;
    if is_consumable(module.path()) {
        return Err(VfsError::NotFound);
    }
    let bytes = module_bytes(module);
    if !suffix.starts_with(boot_model_logic::VOCAB_DIR) {
        return Ok(bytes);
    }
    if !boot_model_logic::is_pinned(VOCAB_PINS, suffix) {
        serial_println!("BOOTMODULE refused path={} reason=unpinned", suffix);
        return Err(VfsError::InsufficientRights);
    }
    let state = VERIFY.get(index).ok_or(VfsError::NotFound)?;
    match state.load(Ordering::Acquire) {
        VERIFIED => return Ok(bytes),
        REFUSED => return Err(VfsError::Integrity),
        _ => {}
    }
    let digest = *blake3::hash(bytes).as_bytes();
    let verdict = boot_model_logic::verdict(VOCAB_PINS, suffix, &digest);
    let expected = if verdict == Verdict::Match {
        digest
    } else {
        boot_model_logic::expected(VOCAB_PINS, suffix).unwrap_or([0; 32])
    };
    let expected = boot_model_logic::hex_digest(&expected);
    let got = boot_model_logic::hex_digest(&digest);
    serial_println!(
        "BOOTMODULE digest path={} bytes={} expected={} got={} verified={}",
        suffix,
        bytes.len(),
        core::str::from_utf8(&expected).unwrap_or("?"),
        core::str::from_utf8(&got).unwrap_or("?"),
        verdict == Verdict::Match
    );
    if verdict == Verdict::Match {
        state.store(VERIFIED, Ordering::Release);
        Ok(bytes)
    } else {
        state.store(REFUSED, Ordering::Release);
        Err(VfsError::Integrity)
    }
}

/// An ordinary module whose path ends with `suffix`. Consumable modules are
/// never returned here, whether taken or not.
pub fn find(suffix: &str) -> Option<&'static [u8]> {
    let module = modules().iter().find(|m| m.path().ends_with(suffix))?;
    if is_consumable(module.path()) {
        return None;
    }
    Some(module_bytes(module))
}

/// Take the consumable module whose path ends with `suffix`, once. `None` if
/// there is none, it is not consumable, or it was already taken.
pub fn take(suffix: &str) -> Option<ModuleLease> {
    let (index, module) = modules()
        .iter()
        .enumerate()
        .find(|(_, m)| m.path().ends_with(suffix))?;
    if !is_consumable(module.path()) || index >= STATE.len() {
        return None;
    }
    STATE[index]
        .compare_exchange(PRESENT, TAKEN, Ordering::AcqRel, Ordering::Acquire)
        .ok()?;
    Some(ModuleLease {
        index,
        bytes: module_bytes(module),
    })
}

/// Why a taken module's pages were not admitted. The module stays taken
/// (tombstoned) either way, and unadmitted pages stay retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReclaimError {
    /// The memmap or executable could not be read.
    NoBootInfo,
    /// More memmap entries than [`MAX_MEMMAP_ENTRIES`].
    TooManyEntries,
    /// The planner refused the pages.
    Plan(PlanRefusal),
    /// The allocator refused the zone.
    Admit(AdmitError),
}

/// Exclusive ownership of a taken consumable module.
pub struct ModuleLease {
    index: usize,
    /// Private: only ever lent out with the lease's lifetime.
    bytes: &'static [u8],
}

impl ModuleLease {
    /// The module's bytes, for as long as the lease is held.
    pub fn bytes(&self) -> &[u8] {
        self.bytes
    }

    /// Plan and admit the module's whole pages as a new allocator zone.
    /// Consumes the lease: no borrow of [`Self::bytes`] can outlive this call.
    pub fn reclaim(self) -> Result<u64, ReclaimError> {
        let result = self.plan().and_then(|pages| {
            crate::mm::with_frame_alloc(|fa| fa.admit_external_region(pages.start, pages.end))
                .map_err(ReclaimError::Admit)
        });
        let module = modules()[self.index];
        serial_println!(
            "BOOTMOD reclaim path={} bytes={} result={:?}",
            module.path(),
            self.bytes.len(),
            result
        );
        result
    }

    /// The forged-plan control: the same plan with every memmap entry that
    /// touches the module re-typed `RESERVED`. Must be refused.
    #[cfg(feature = "neo-test-boot-module-reclaim")]
    pub fn debug_forged_plan(&self) -> Result<Span, ReclaimError> {
        self.plan_with(true)
    }

    fn plan(&self) -> Result<Span, ReclaimError> {
        self.plan_with(false)
    }

    fn plan_with(&self, forge: bool) -> Result<Span, ReclaimError> {
        let hhdm = crate::mm::hhdm_offset();
        let extent =
            |m: &limine::file::File| -> PhysExtent { crate::limine_file_data_extent(hhdm, m) };
        let all = modules();
        let this = extent(all[self.index]);

        let entries = crate::MEMMAP_REQUEST
            .response()
            .ok_or(ReclaimError::NoBootInfo)?
            .entries();
        if entries.len() > MAX_MEMMAP_ENTRIES {
            return Err(ReclaimError::TooManyEntries);
        }
        let mut memmap = [MapEntry {
            base: 0,
            length: 0,
            kind: 0,
        }; MAX_MEMMAP_ENTRIES];
        for (out, e) in memmap.iter_mut().zip(entries.iter()) {
            let touches = e.base < this.end && this.start < e.base.saturating_add(e.length);
            *out = MapEntry {
                base: e.base,
                length: e.length,
                kind: if forge && touches { 1 } else { e.type_ },
            };
        }

        let (exec_phys, _) = crate::executable_address().ok_or(ReclaimError::NoBootInfo)?;
        let exec_end = exec_phys
            .checked_add(crate::kernel_image_size())
            .ok_or(ReclaimError::Plan(PlanRefusal::Overflow))?;

        let mut others = [Span::new(0, 0); crate::MAX_BOOT_MODULE_EXTENTS];
        for (i, (out, m)) in others.iter_mut().zip(all.iter()).enumerate() {
            if i != self.index {
                let e = extent(m);
                *out = Span::new(e.start, e.end);
            }
        }

        plan_module_reclaim(
            Span::new(this.start, this.end),
            &memmap[..entries.len()],
            Span::new(exec_phys, exec_end),
            &others[..all.len()],
        )
        .map_err(ReclaimError::Plan)
    }
}

/// F7: the staged module list, logged at early boot before the capacity check
/// and before any module byte is touched (sizes come from the file metadata,
/// as the HHDM extent collection reads them).
pub fn log_boot_modules(hhdm_offset: u64) {
    let all = modules();
    serial_println!(
        "BOOTMOD count={} capacity={}",
        all.len(),
        crate::MAX_BOOT_MODULE_EXTENTS
    );
    for module in all {
        let extent = crate::limine_file_data_extent(hhdm_offset, module);
        serial_println!(
            "BOOTMOD module path={} bytes={} phys=0x{:x} consumable={}",
            module.path(),
            extent.end - extent.start,
            extent.start,
            is_consumable(module.path())
        );
    }
}

/// The pins this kernel holds models to. A test kernel also accepts the
/// hermetic fixture; it still holds the real 0.5B to the real digest.
#[cfg(feature = "neo-test")]
pub(crate) const PINS: &[ModelPin] = boot_model_logic::TEST_PINS;
#[cfg(not(feature = "neo-test"))]
pub(crate) const PINS: &[ModelPin] = boot_model_logic::PRODUCT_PINS;

/// B1c: the vocabularies this kernel serves from `/boot/vocab/`.
#[cfg(feature = "neo-test")]
pub(crate) const VOCAB_PINS: &[ModelPin] = boot_model_logic::TEST_VOCAB_PINS;
#[cfg(not(feature = "neo-test"))]
pub(crate) const VOCAB_PINS: &[ModelPin] = boot_model_logic::PRODUCT_VOCAB_PINS;

const UNVERIFIED: u8 = 0;
const VERIFIED: u8 = 1;
const REFUSED: u8 = 2;

/// B1c: the cached digest verdict per module index (vocab modules only).
static VERIFY: [AtomicU8; crate::MAX_BOOT_MODULE_EXTENTS] =
    [const { AtomicU8::new(UNVERIFIED) }; crate::MAX_BOOT_MODULE_EXTENTS];

/// B1b: load the model a `/boot/models/<file>` path names, from its boot
/// module.
///
/// Order is the contract:
/// 1. an unknown or unpinned path is refused **before** the take, so nothing
///    is consumed (`ENOENT` / `EPERM`);
/// 2. the module is taken once (a second load finds it tombstoned: `ENOENT`);
/// 3. every byte is hashed and judged against the pin before any byte is used.
///    A mismatch builds nothing (`EBADMSG`, Arthur's "damaged");
/// 4. the model is built from the lease's bytes (weights are copied into
///    tensor storage, so the model keeps no reference to them);
/// 5. the lease is consumed by `reclaim`, so no borrow of the module outlives
///    it. A refused reclaim keeps the pages retained and does not fail the
///    load.
///
/// The module is reclaimed whatever the verdict: a damaged model is never
/// going to be used, so its frames are worth having back.
pub fn load_model(
    path: &str,
    owner: crate::sched::process::ProcessId,
) -> Result<crate::ai::model::Model, i64> {
    let module = boot_model_logic::module_suffix(path).ok_or(ENOENT)?;
    if !boot_model_logic::is_pinned(PINS, module) {
        serial_println!("BOOTMODEL refused path={} reason=unpinned", module);
        return Err(EPERM);
    }
    crate::mm::boot_census("model_before_take");
    let Some(lease) = take(module) else {
        serial_println!("BOOTMODEL absent path={}", module);
        return Err(ENOENT);
    };
    let len = lease.bytes().len();
    serial_println!("BOOTMODEL take path={} bytes={}", module, len);

    #[cfg(feature = "freeze-watchdog")]
    crate::watchdog::set_phase(crate::watchdog::Phase::LoadHash);
    let digest = *blake3::hash(lease.bytes()).as_bytes();
    let verdict = boot_model_logic::verdict(PINS, module, &digest);
    let got = boot_model_logic::hex_digest(&digest);
    // On a match, the pin that matched (the digest itself). On a mismatch,
    // the module's first pin: the real model's, which is what silicon expects.
    let expected = if verdict == Verdict::Match {
        digest
    } else {
        boot_model_logic::expected(PINS, module).unwrap_or([0; 32])
    };
    let expected = boot_model_logic::hex_digest(&expected);
    serial_println!(
        "BOOTMODEL digest path={} expected={} got={} match={}",
        module,
        core::str::from_utf8(&expected).unwrap_or("?"),
        core::str::from_utf8(&got).unwrap_or("?"),
        verdict == Verdict::Match
    );
    if verdict != Verdict::Match {
        // Unpinned cannot happen here (checked before the take); either way
        // nothing is built.
        let _ = lease.reclaim();
        crate::mm::boot_census("model_damaged_reclaimed");
        return Err(EBADMSG);
    }

    #[cfg(feature = "freeze-watchdog")]
    crate::watchdog::set_phase(crate::watchdog::Phase::LoadBuild);
    // The tick discriminator holds the load here, between the hash and the
    // build, and measures what CPU 0 does meanwhile.
    #[cfg(feature = "neo-test-boot-model-ticks")]
    crate::test_harness::boot_model_ticks_park();
    let built = crate::ai::model::Model::from_neom_bytes(
        lease.bytes(),
        crate::fs::FileProvenance::boot_module(),
        Some(owner),
    );
    crate::mm::boot_census("model_built");
    #[cfg(feature = "freeze-watchdog")]
    crate::watchdog::set_phase(crate::watchdog::Phase::LoadDone);
    let reclaimed = lease.reclaim();
    crate::mm::boot_census("model_reclaimed");
    match built {
        Ok(model) => {
            serial_println!(
                "BOOTMODEL loaded path={} bytes={} reclaimed_pages={}",
                module,
                len,
                reclaimed.map_or(0, |p| p)
            );
            Ok(model)
        }
        Err(e) => {
            serial_println!("BOOTMODEL load_failed path={} err={:?}", module, e);
            Err(crate::ai::model::model_errno(e))
        }
    }
}

/// One ordinary boot module, served read-only from its `'static` bytes.
struct ModuleInode {
    bytes: &'static [u8],
}

impl Inode for ModuleInode {
    fn kind(&self) -> InodeKind {
        InodeKind::Regular
    }

    fn available_rights(&self) -> FileRights {
        FileRights::READ
    }

    fn read_at(&self, offset: usize, out: &mut [u8]) -> Result<usize, VfsError> {
        let Some(rest) = self.bytes.get(offset..) else {
            return Ok(0);
        };
        let n = out.len().min(rest.len());
        out[..n].copy_from_slice(&rest[..n]);
        Ok(n)
    }

    fn write_at(&self, _offset: usize, _input: &[u8]) -> Result<usize, VfsError> {
        Err(VfsError::InsufficientRights)
    }

    fn byte_len(&self) -> Option<u64> {
        Some(self.bytes.len() as u64)
    }
}

/// `/boot`: `lookup("vocab/x")` serves the ordinary module whose path ends
/// with `/vocab/x`, once it passed its pin ([`find_verified`]). Consumable
/// modules are never served, so a model can never be read here, taken or not.
struct BootFs;

impl MountedFs for BootFs {
    fn root(&self) -> Result<Arc<dyn Inode>, VfsError> {
        Ok(crate::fs::empty_directory_inode())
    }

    fn lookup(&self, relative: &str) -> Result<Arc<dyn Inode>, VfsError> {
        if relative.is_empty() {
            return Ok(crate::fs::empty_directory_inode());
        }
        if relative.contains("..") {
            return Err(VfsError::NotFound);
        }
        let mut suffix = alloc::string::String::from("/");
        suffix.push_str(relative);
        let bytes = find_verified(&suffix)?;
        Ok(Arc::new(ModuleInode { bytes }))
    }
}

/// Mount `/boot` (read-only). Called once after `fs::init`.
pub fn mount_boot_fs() {
    match crate::fs::install_read_only_root_child("boot", Arc::new(BootFs)) {
        Ok(()) => {
            serial_println!("[ kernel ] fs: mounted read-only /boot module namespace (B1b)");
        }
        Err(e) => {
            serial_println!("[ kernel ] fs: /boot not mounted: {:?}", e);
        }
    }
}

/// Zenbook ask-freeze: every pinned boot model, verified and built once at
/// boot, before any user process exists. `sys_model_load` of a
/// `/boot/models/` path then hands out a handle to the built model (each
/// Arthur gets the same one, instantly) instead of loading in the syscall.
///
/// Why at boot, and on the inference stack:
/// - the load hashed the whole module inside the syscall at `IF=0`, so CPU 0
///   took no tick and no xHCI interrupt for its whole length (the frozen
///   desktop);
/// - BLAKE3 alone needs 39,536 B of stack at the 0.5B's 485 MB
///   (`boot_model_load_stack`), more than a thread's unguarded 32 KiB box: the
///   load overran it into the heap. The inference stack is 256 KiB with a
///   guard page, and runs its closure with interrupts on (CP-74).
///
/// Lock safety at `IF=1`: the build takes `ARENA`, `DEDUP` and `REGISTRY`
/// (plain spin locks). A preempted holder can only be waited on by another
/// thread, and none of them takes those locks until userland exists; the
/// frame allocator and the heap mask interrupts themselves.
#[cfg(feature = "boot-model-preload")]
pub struct Preloaded {
    /// The module suffix (`/models/<file>`).
    pub module: &'static str,
    pub result: Result<Arc<crate::ai::model::Model>, i64>,
    /// The inference stack's depth for the load, in bytes.
    pub stack_depth: usize,
    /// OT-87 R2: built from `/neofs/<file>` (no boot module named it).
    #[cfg(feature = "model-from-neofs")]
    pub from_neofs: bool,
}

#[cfg(feature = "boot-model-preload")]
static PRELOADED: spin::Once<alloc::vec::Vec<Preloaded>> = spin::Once::new();

/// Whether the consumable module ending in `suffix` is still untaken.
#[cfg(feature = "boot-model-preload")]
fn untaken(suffix: &str) -> bool {
    modules()
        .iter()
        .enumerate()
        .find(|(_, m)| m.path().ends_with(suffix))
        .is_some_and(|(index, m)| {
            is_consumable(m.path())
                && STATE
                    .get(index)
                    .is_some_and(|s| s.load(Ordering::Acquire) == PRESENT)
        })
}

/// Preload every pinned, present boot model. Runs once, on the boot thread,
/// after the scheduler and before userland. A model whose pinned vocab is
/// absent or fails its pin is left untaken, as Arthur's `compose` leaves it
/// (B1c F8).
#[cfg(feature = "boot-model-preload")]
pub fn preload() {
    PRELOADED.call_once(|| {
        let mut done: alloc::vec::Vec<Preloaded> = alloc::vec::Vec::new();
        // OT-87 R4.3: the Install entry (it loads the install bundle) leaves
        // the consumable model untaken, for the installer to stream to NeoFS
        // (THINK §4.3 item 4, §6.1).
        #[cfg(feature = "installer")]
        if find(crate::installer::BUNDLE_MODULE).is_some() {
            crate::serial_println!("BOOTMODULE preload skipped reason=install_bundle_present");
            return done;
        }
        // Test kernels pin two digests for one module path.
        let mut seen: alloc::vec::Vec<&str> = alloc::vec::Vec::new();
        for pin in PINS {
            let module = pin.module;
            if seen.contains(&module) || !untaken(module) {
                continue;
            }
            seen.push(module);
            // The model's vocab, in whichever form the image staged (text
            // sidecar or NEOVOCB1): every staged form must verify, and at
            // least one must be staged.
            let mut vocab_ok = false;
            let mut vocab_bad = false;
            let mut vocab_pinned = false;
            for vocab in VOCAB_PINS
                .iter()
                .filter(|v| boot_model_logic::vocab_pairs_model(module, v.module))
            {
                vocab_pinned = true;
                match find_verified(vocab.module) {
                    Ok(_) => vocab_ok = true,
                    Err(VfsError::NotFound) => {}
                    Err(_) => vocab_bad = true,
                }
            }
            if vocab_pinned && (vocab_bad || !vocab_ok) {
                serial_println!("BOOTMODEL preload path={} skipped=vocab", module);
                continue;
            }
            let path = alloc::format!("/boot{}", module);
            let ticks0 = crate::arch::x86_64::timer::TICKS.load(Ordering::Acquire);
            serial_println!("BOOTMODEL preload path={} stage=start", module);
            let (result, report) = crate::mm::inference_stack::run_checked(|| {
                load_model(&path, crate::sched::process::KERNEL_PID).map(Arc::new)
            });
            let ticks = crate::arch::x86_64::timer::TICKS
                .load(Ordering::Acquire)
                .wrapping_sub(ticks0);
            let result = crate::mm::inference_stack::finish(result, report);
            serial_println!(
                "BOOTMODEL preload path={} ok={} errno={} stack_depth={} ticks={}",
                module,
                result.is_ok(),
                result.as_ref().err().copied().unwrap_or(0),
                report.depth,
                ticks
            );
            done.push(Preloaded {
                module,
                result,
                stack_depth: report.depth,
                #[cfg(feature = "model-from-neofs")]
                from_neofs: false,
            });
        }
        #[cfg(feature = "model-from-neofs")]
        preload_neofs(&mut done);
        done
    });
}

/// Whether any boot module, consumable or not, taken or not, ends in `suffix`.
#[cfg(feature = "model-from-neofs")]
fn staged(suffix: &str) -> bool {
    modules().iter().any(|m| m.path().ends_with(suffix))
}

/// OT-87 R2: the second source. For every pinned model that no boot module
/// names (a boot module, even a damaged one, always wins), build
/// `/neofs/<file>` if it is there, on the inference stack. Its vocab is held
/// to the same rule as the boot module's: every staged form verifies, and at
/// least one is staged.
#[cfg(feature = "model-from-neofs")]
fn preload_neofs(done: &mut alloc::vec::Vec<Preloaded>) {
    let mut seen: alloc::vec::Vec<&str> = alloc::vec::Vec::new();
    for pin in PINS {
        let module = pin.module;
        if seen.contains(&module) || staged(module) {
            continue;
        }
        seen.push(module);
        let Some(name) = boot_model_logic::neofs_name(module) else {
            continue;
        };
        let path = alloc::format!("{}{}", boot_model_logic::NEOFS_DIR, name);
        if crate::fs::open_root_temporary(
            &path,
            crate::fs::OPEN_ROOT_TEMPORARY,
            crate::fs::FileRights::READ,
        )
        .is_err()
        {
            serial_println!("NEOFSMODEL preload path={} absent", path);
            continue;
        }
        let ticks0 = crate::arch::x86_64::timer::TICKS.load(Ordering::Acquire);
        serial_println!("NEOFSMODEL preload path={} stage=start", path);
        // The vocab check hashes its file, so it runs on the inference stack
        // too (BLAKE3's depth; `boot_model_load_stack`).
        let (result, report) = crate::mm::inference_stack::run_checked(|| {
            if !neofs_vocab_ok(module) {
                return None;
            }
            Some(crate::neofs_model::load(&path, crate::sched::process::KERNEL_PID).map(Arc::new))
        });
        let ticks = crate::arch::x86_64::timer::TICKS
            .load(Ordering::Acquire)
            .wrapping_sub(ticks0);
        let Some(result) = crate::mm::inference_stack::finish(result, report) else {
            serial_println!("NEOFSMODEL preload path={} skipped=vocab", path);
            continue;
        };
        serial_println!(
            "NEOFSMODEL preload path={} ok={} errno={} stack_depth={} ticks={}",
            path,
            result.is_ok(),
            result.as_ref().err().copied().unwrap_or(0),
            report.depth,
            ticks
        );
        done.push(Preloaded {
            module,
            result,
            stack_depth: report.depth,
            from_neofs: true,
        });
    }
}

/// The NeoFS vocab rule for `module`'s model: every staged form passes its
/// pin, and at least one form is staged.
#[cfg(feature = "model-from-neofs")]
fn neofs_vocab_ok(module: &str) -> bool {
    let mut ok = false;
    let mut bad = false;
    let mut pinned = false;
    // Test kernels pin two digests for one name; each file is checked once.
    let mut seen: alloc::vec::Vec<&str> = alloc::vec::Vec::new();
    for vocab in VOCAB_PINS
        .iter()
        .filter(|v| boot_model_logic::vocab_pairs_model(module, v.module))
    {
        let Some(name) = boot_model_logic::neofs_name(vocab.module) else {
            continue;
        };
        pinned = true;
        if seen.contains(&name) {
            continue;
        }
        seen.push(name);
        let path = alloc::format!("{}{}", boot_model_logic::NEOFS_DIR, name);
        match crate::fs::open_root_temporary(
            &path,
            crate::fs::OPEN_ROOT_TEMPORARY,
            crate::fs::FileRights::READ,
        ) {
            Ok(file) => match crate::neofs_model::check(
                &file,
                &path,
                boot_model_logic::NeofsPin::Vocab(vocab.module),
            ) {
                Ok(_) => ok = true,
                Err(_) => bad = true,
            },
            Err(VfsError::NotFound) => {}
            Err(_) => bad = true,
        }
    }
    !pinned || (ok && !bad)
}

/// The preloaded result for a `/boot/models/` path, if that model was
/// preloaded. `None`: not preloaded (absent, or its vocab failed), and the
/// caller falls back to [`load_model`].
#[cfg(feature = "boot-model-preload")]
pub fn preloaded(path: &str) -> Option<Result<Arc<crate::ai::model::Model>, i64>> {
    let module = boot_model_logic::module_suffix(path)?;
    PRELOADED
        .get()?
        .iter()
        .find(|p| {
            #[cfg(feature = "model-from-neofs")]
            if p.from_neofs {
                return false;
            }
            p.module == module
        })
        .map(|p| p.result.clone())
}

/// OT-87 R2: the preloaded result for a pinned `/neofs/` model path. `None`:
/// not preloaded from NeoFS, and the caller loads it in the syscall.
#[cfg(feature = "model-from-neofs")]
pub fn preloaded_neofs(path: &str) -> Option<Result<Arc<crate::ai::model::Model>, i64>> {
    let boot_model_logic::NeofsPin::Model(module) = crate::neofs_model::pin_for(path)? else {
        return None;
    };
    PRELOADED
        .get()?
        .iter()
        .find(|p| p.from_neofs && p.module == module)
        .map(|p| p.result.clone())
}

/// The preloads, for the census and the tests.
#[cfg(feature = "boot-model-preload")]
#[allow(dead_code)]
pub fn preloads() -> &'static [Preloaded] {
    PRELOADED.get().map_or(&[], |v| v.as_slice())
}
