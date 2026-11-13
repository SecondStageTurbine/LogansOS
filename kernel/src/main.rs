// SPDX-License-Identifier: MPL-2.0
//! NeoPlutarch Kernel — entry point.
//!
//! This is the very first Rust code that runs after Limine hands control over.
//! Our job in Phase 0 is intentionally small:
//!   1. Wire up serial I/O so we can `println!` from the kernel.
//!   2. Pull the framebuffer from Limine and draw the boot logo.
//!   3. Halt forever (Phase 0 has no scheduler).
//!
//! Phase 1 will add: GDT, IDT, paging, frame allocator, heap, scheduler.
//! Phase 2 will move drivers to userspace and we'll start removing code from here.
//!
//! Module layout:
//!   arch/   architecture-specific bring-up (CPU, port I/O)
//!   serial  16550 UART driver, kernel debug output
//!   fb      framebuffer access + bitmap font rendering
//!   boot    boot sequence rendering ("the world is listening.")
//!   panic   panic handler
//!
//! Everything here is provisional and will be reorganized as the kernel grows.

#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
// The boot banner's build line reports what this kernel was compiled with.
#![feature(cfg_ub_checks, cfg_overflow_checks)]

#[cfg(all(
    feature = "neo-test-arthur-compositor",
    feature = "framebuffer-console"
))]
compile_error!("integrated Arthur must render through its G1 surface, not framebuffer-console");

extern crate alloc;

// Phase 7.5 BUILD-V: ACPI/DMAR discovery for the VT-d driver. Compiled only
// under `vtd` (the kill-switch); forward-declared APIs (`with_dmar`, etc.) land
// with the `iommu` driver in the same build.
// CP-69: the HPET / PM timer for `lapic-timer`. OT-85: the FADT's
// CMOS-clock fields for the `rtc-epoch` seed.
#[cfg(any(
    feature = "vtd",
    feature = "lapic-timer",
    feature = "rtc-epoch",
    feature = "ps2-keyboard",
    feature = "i2c-hid-touchpad",
    feature = "power-control",
    feature = "boot-time"
))]
#[allow(dead_code)]
mod acpi;
mod ai;
mod arch;
// L7.1: the boot-time numbers (`SYS_BOOT_TIMES`).
// Its desktop-start stamp is the desktop session's (and the scenario's).
#[cfg(feature = "boot-time")]
#[allow(dead_code)]
mod boot_time;
// Its decoder and milestone line are libneo's.
#[cfg(feature = "atrium-compositor-g1")]
mod atrium;
#[cfg(feature = "model-boot-module")]
mod boot_modules;
#[cfg(feature = "boot-time")]
#[allow(dead_code)]
mod boot_time_logic;
// One of its two pin tables is live per build (test or product).
#[cfg(feature = "model-boot-module")]
#[allow(dead_code)]
mod boot_model_logic;
#[allow(dead_code)] // driver probe/debug APIs are forward-declared across hardware phases
mod drivers;
// OT-87 R2: Arthur's model from the installed NeoFS volume.
mod errno;
#[allow(dead_code)] // VFS and filesystem mutation APIs are staged ahead of callers
mod fs;
mod init;
#[allow(dead_code)] // IPC capability helpers include future syscall/debug surfaces
mod ipc;
#[allow(dead_code)] // loader inspection helpers are kept for test and policy phases
mod loader;
#[allow(dead_code)] // log-sink helpers are enabled selectively by scenarios
mod log;
#[allow(dead_code)] // allocator/MM diagnostics are exported for phase gates and tests
mod mm;
#[cfg(feature = "model-from-neofs")]
mod neofs_model;
// OT-87 R4.3: the install bundle's format (pure, shared with xtask) and the
// installer that writes it.
#[cfg(feature = "installer")]
#[allow(dead_code)] // the writer half is xtask's; the kernel reads
mod install_bundle_logic;
#[cfg(feature = "installer")]
mod install_ui_logic;
#[cfg(feature = "installer")]
#[allow(dead_code)] // the scenarios' hooks (`test_set_no_flush`) and accessors
mod installer;
#[cfg(feature = "installer")]
mod installer_ui;
#[cfg(any(feature = "installer", feature = "usb-persistence"))]
mod limine_file_logic;
// The boot medium: the kernel file, its disk's GPT GUID and (OT-100 R3) the
// entry's command line. Shared by the installer and the persistence mount.
#[cfg(any(feature = "installer", feature = "usb-persistence"))]
#[allow(dead_code)] // each feature uses part of it
mod boot_medium;
// OT-100 R3: the persistence area's pure decisions (host-tested).
#[allow(dead_code)] // forward-declared APIs for future phases; callers land later
mod net;
mod observability;
mod panic;
#[cfg(feature = "usb-persistence")]
#[allow(dead_code)] // the mount uses the rest
mod persist_logic;
// Phase 10.1 BUILD-10.1: kernel CSPRNG (entropy + ChaCha20 DRBG + KATs).
// Compiled only under `kernel-rng` (the kill-switch); default-off ≡ today.
#[cfg(feature = "kernel-rng")]
mod rng;
#[cfg(feature = "neo-test")]
mod scenario;
#[allow(dead_code)] // scheduler/process helpers expose future and test-only surfaces
mod sched;
mod sync;
// OT-83: ring-3 exception → kill-the-process routing (host-tested via `#[path]`).
#[cfg(feature = "neo-test")]
mod test_harness;
mod user_fault_logic;
#[cfg(feature = "freeze-watchdog")]
mod watchdog;
#[cfg(feature = "freeze-watchdog")]
mod watchdog_timeline_logic;

#[cfg(not(feature = "neo-test"))]
use core::sync::atomic::{AtomicBool, Ordering};
use limine::BaseRevision;
use limine::request::{
    ExecutableAddressRequest, FramebufferRequest, HhdmRequest, MemmapRequest, ModulesRequest,
};
use limine::{RequestsEndMarker, RequestsStartMarker};

// === Limine boot protocol requests ===
//
// These static requests are what Limine scans for in our binary. They must
// be `#[used]` and reside in linker-visible sections, otherwise LTO will
// happily drop them and the bootloader won't know what we want.

// Request Limine base revision 3 explicitly. The crate's `BaseRevision::new()`
// asks for revision 6 (the crate's MAX_SUPPORTED), which Limine v8.6 doesn't
// serve — and when negotiation falls back, framebuffer pointers may end up as
// physical addresses (revision 0 semantics) instead of higher-half mapped,
// triple-faulting the first framebuffer write. Revision 3 is the highest the
// v8.x bootloader supports today and gives us HHDM-mapped pointers.
#[used]
#[unsafe(link_section = ".requests")]
static BASE_REVISION: BaseRevision = BaseRevision::with_revision(3);

#[used]
#[unsafe(link_section = ".requests")]
static FRAMEBUFFER_REQUEST: FramebufferRequest = FramebufferRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
static HHDM_REQUEST: HhdmRequest = HhdmRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
static MEMMAP_REQUEST: MemmapRequest = MemmapRequest::new();

/// L7.1: the raw UEFI memory map, whose types tell firmware RAM from MMIO
/// (Limine's own map folds both into RESERVED).
#[cfg(feature = "boot-time")]
#[used]
#[unsafe(link_section = ".requests")]
static EFI_MEMMAP_REQUEST: limine::request::EfiMemmapRequest =
    limine::request::EfiMemmapRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
static MODULE_REQUEST: ModulesRequest = ModulesRequest::new();

#[used]
#[unsafe(link_section = ".requests")]
static EXECUTABLE_ADDRESS_REQUEST: ExecutableAddressRequest = ExecutableAddressRequest::new();

// OT-87 R4.3: the kernel file as Limine loaded it, so the installer writes the
// running kernel's exact bytes to slot A, and the boot medium's GPT disk GUID
// (THINK §4.3 item 3, §2a). OT-100 R3 identifies the boot stick by that GUID.
// Gated by `installer` / `usb-persistence`, so the request vanishes without
// them.
#[cfg(any(feature = "installer", feature = "usb-persistence"))]
#[used]
#[unsafe(link_section = ".requests")]
static EXECUTABLE_FILE_REQUEST: limine::request::ExecutableFileRequest =
    limine::request::ExecutableFileRequest::new();

// Phase 7.5 BUILD-V: the ACPI RSDP pointer (for the DMAR table). Base revision
// 3 returns this as a PHYSICAL address (see limine `RsdpRespData`), so the acpi
// parser HHDM-maps it. Gated by `vtd` so the request vanishes in pre-VT-d builds.
#[cfg(any(
    feature = "vtd",
    feature = "lapic-timer",
    feature = "rtc-epoch",
    feature = "ps2-keyboard",
    feature = "i2c-hid-touchpad",
    feature = "power-control"
))]
#[used]
#[unsafe(link_section = ".requests")]
static RSDP_REQUEST: limine::request::RsdpRequest = limine::request::RsdpRequest::new();

/// The RSDP's physical address, or 0 (L7.1's harness scenario).
#[cfg(all(feature = "boot-time", feature = "neo-test"))]
pub(crate) fn rsdp_phys() -> u64 {
    RSDP_REQUEST
        .response()
        .map(|r| r.address as u64)
        .unwrap_or(0)
}

// Markers tell Limine where our requests block begins and ends. Without
// these, request scanning is undefined.
#[used]
#[unsafe(link_section = ".requests_start_marker")]
static _START_MARKER: RequestsStartMarker = RequestsStartMarker::new();

#[used]
#[unsafe(link_section = ".requests_end_marker")]
static _END_MARKER: RequestsEndMarker = RequestsEndMarker::new();

const MAX_BOOT_MODULE_EXTENTS: usize = 8;

#[repr(C)]
struct LimineFileDataPrefix {
    revision: u64,
    address: *mut (),
    size: u64,
}

unsafe extern "C" {
    static __kernel_image_start: u8;
    static __kernel_image_end: u8;
}

/// Set to `true` once we've finished early bring-up. Used by the panic
/// handler to decide whether it can safely use the framebuffer.
#[cfg(not(feature = "neo-test"))]
static KERNEL_READY: AtomicBool = AtomicBool::new(false);

/// Limine's load-time executable base. BUILD-3 uses this both for serial
/// verification and for the KASLR policy smoke test.
pub fn executable_address() -> Option<(u64, u64)> {
    let response = EXECUTABLE_ADDRESS_REQUEST.response()?;
    Some((response.physical_base, response.virtual_base))
}

fn kernel_image_size() -> u64 {
    // SAFETY: linker script defines both symbols for the loaded kernel image;
    // taking their addresses does not dereference memory.
    let start = unsafe { &__kernel_image_start as *const u8 as u64 };
    // SAFETY: linker script defines both symbols for the loaded kernel image;
    // taking their addresses does not dereference memory.
    let end = unsafe { &__kernel_image_end as *const u8 as u64 };
    end.checked_sub(start)
        .expect("kernel image linker symbols are inverted")
}

fn hhdm_pointer_to_phys(hhdm_offset: u64, virt: u64, label: &str) -> u64 {
    virt.checked_sub(hhdm_offset)
        .unwrap_or_else(|| panic!("{label}: pointer 0x{virt:016x} below HHDM offset"))
}

fn limine_file_data_extent(hhdm_offset: u64, file: &limine::file::File) -> mm::PhysExtent {
    // SAFETY: limine::file::File is #[repr(C)] in limine 0.6.x, and this
    // prefix mirrors its first three fields: revision, data address, size.
    // Reading only this metadata avoids constructing a byte slice until after
    // mm::init has verified and repaired HHDM coverage for the module bytes.
    let prefix = unsafe { &*(file as *const _ as *const LimineFileDataPrefix) };
    let phys_start = hhdm_pointer_to_phys(hhdm_offset, prefix.address as u64, "limine module data");
    let phys_end = phys_start
        .checked_add(prefix.size)
        .expect("limine module span overflow");
    mm::PhysExtent::new(phys_start, phys_end)
}

/// Look up a Limine module by path suffix (e.g., "/neo-worker"). Returns
/// the module's bytes mapped via HHDM, with `'static` lifetime — Limine
/// guarantees module data lives for the kernel's entire lifetime.
///
/// Used by `process::spawn_user_by_name` (driven from `sys_spawn` in the
/// syscall handler) to load additional userspace processes at runtime.
pub fn find_module_by_path_suffix(suffix: &str) -> Option<&'static [u8]> {
    // B1a: consumable `/models/` modules are never lent out as `'static`
    // slices; they can only be taken once (`boot_modules::take`).
    #[cfg(feature = "model-boot-module")]
    {
        boot_modules::find(suffix)
    }
    #[cfg(not(feature = "model-boot-module"))]
    {
        let response = MODULE_REQUEST.response()?;
        let module = response
            .modules()
            .iter()
            .find(|m| m.path().ends_with(suffix))?;
        let slice = module.data();
        // SAFETY: Limine documents that module data lives for the kernel's
        // entire lifetime in HHDM-mapped memory.
        Some(unsafe { core::slice::from_raw_parts(slice.as_ptr(), slice.len()) })
    }
}

/// OT-61 browser C0.5: spawn the **real-`std`-linked** smoke ELF (built for
/// `x86_64-unknown-neoplutarch` with `-Z build-std=std` against the rust-src
/// overlay, staged as `/neo-std-real-smoke`) through the SAME ELF-loader /
/// `spawn_user` / reap path as `neo-init`, and return its exit code. This is
/// the FIRST real-`std` ELF through that path — a genuine integration test of
/// the loader/process surface (no new syscall; rides the existing spawn path).
/// A loud-stub fire or a loader gap surfaces as a non-zero exit / process fault
/// (a valid, self-diagnosing C0.5 finding).
#[cfg(feature = "neo-test-std-real-smoke")]
pub(crate) fn spawn_std_real_smoke() -> i32 {
    use sched::process;

    let bytes = find_module_by_path_suffix("/neo-std-real-smoke")
        .expect("limine: no /neo-std-real-smoke module (did limine.conf/iso forget it?)");
    serial_println!(
        "[ kernel ] elf: std-real-smoke module at 0x{:016x} size={} bytes",
        bytes.as_ptr() as usize,
        bytes.len()
    );
    let pid = process::spawn_user("std-real-smoke", bytes, &[])
        .expect("spawn_user(std-real-smoke) failed — C0.5 ELF-loader gap");
    serial_println!("[ kernel ] proc: std-real-smoke spawned pid={}", pid.0);

    while process::any_user_process() {
        sched::yield_now();
        sched::reap_and_drain();
    }
    // A drop deferred by the last pass's teardown (the #226 bounce).
    sched::reap_and_drain();
    let exit_code = process::take_spawned_exit_code(pid).unwrap_or_else(|| {
        serial_println!(
            "[ kernel ] proc: missing std-real-smoke exit code (pid={})",
            pid.0
        );
        255
    });
    serial_println!("[ kernel ] proc: std-real-smoke exit code {}", exit_code);
    exit_code
}

/// Phase 1.4 smoke test: locate the `neo-init` module Limine loaded for us,
/// hand it to `process::spawn_user`, then yield until the user process exits
/// (sched::reap drains it; we observe HOG_DONE-style via process registry
/// emptiness).
// The root-capability builder below creates a Vec and then pushes into it, which
// trips `vec_init_then_push`. That shape is deliberate and cannot be a `vec![]`
// literal: every push is cfg-gated, which is the whole point of the
// compositional assembly (CP-34 — mutually-exclusive cfg arrays are what dropped
// an authority in the composed product). The lint's span covers the entire
// statement sequence, so the allow has to sit at function scope.
#[allow(clippy::vec_init_then_push)]
fn spawn_neo_init() -> i32 {
    use sched::process;

    // B1a: through the registry, so a consumable `/models/` module can never
    // be borrowed as neo-init.
    #[cfg(feature = "model-boot-module")]
    let bytes: &'static [u8] =
        boot_modules::find("/neo-init").expect("limine: no module ending with /neo-init found");
    #[cfg(not(feature = "model-boot-module"))]
    let response = MODULE_REQUEST
        .response()
        .expect("limine: no module response (did limine.conf forget module_path?)");

    #[cfg(not(feature = "model-boot-module"))]
    let module = response
        .modules()
        .iter()
        .find(|m| m.path().ends_with("/neo-init"))
        .expect("limine: no module ending with /neo-init found");

    // limine 0.6 File::data() returns the bytes as a slice. The lifetime is
    // tied to the module reference (and therefore to MODULE_REQUEST's static
    // response), which lives for the kernel's lifetime — safe to widen to
    // 'static.
    #[cfg(not(feature = "model-boot-module"))]
    let bytes: &'static [u8] = {
        let slice = module.data();
        // SAFETY: Limine documents that module data lives for the kernel's
        // entire lifetime in HHDM-mapped memory.
        unsafe { core::slice::from_raw_parts(slice.as_ptr(), slice.len()) }
    };
    serial_println!(
        "[ kernel ] elf: neo-init module at 0x{:016x} size={} bytes",
        bytes.as_ptr() as usize,
        bytes.len()
    );

    // Phase 1.5b BUILD-6 e2e leak detector — capture the baseline frame
    // count BEFORE any user-process work. After the wait loop drains, the
    // count must return exactly to baseline; the per-event `frame_audit`
    // calls inside spawn_user / terminate / on_thread_reap surface in-
    // flight accounting, and the debug_assert below catches any leak.
    let baseline_frames = mm::frames_in_use();
    mm::frame_audit("baseline-pre-spawn");

    // Trusted root capabilities are installed only into neo-init, assembled
    // COMPOSITIONALLY (OT-61 C1.1 blocker D): each enabled root authority
    // pushes its cap in a fixed, documented order, so enabling several
    // features at once installs ALL of them — no mutually-exclusive cfg arrays
    // (the bug that made `browser-runtime + net-dial-cap` drop thread
    // authority). Order (stable ABI — children rely on it):
    //   [0] NetDial (when `net-dial-cap`)   — kept at handle 0 as before
    //   [1] GpuSubmit (when any desktop-compositor scenario, or the production
    //       `desktop-session`, is enabled)
    //   [2] BrowserRuntime (when `browser-runtime`)
    //   then Installer (when `installer`), then the RootDirCap (when
    //   `cap-rooted-open`), last
    // `neo-test-gpu-glass-slice6` is a self-contained scenario that inherits
    // ONLY its three GPU-compute caps (it enables none of the above), so it is
    // handled as an exclusive arm. Default-off (no feature) installs nothing.
    // Every cap is a root mint with no userspace self-mint path; children get
    // one only by explicit sys_spawn inheritance/delegation.
    //
    // The NO-root-cap build gets its own arm returning the empty slice. That is
    // NOT a return to the combinatorial arms this blocker removed: the
    // accumulating builder below still handles every combination of *enabled*
    // authorities, so no composition can silently drop one (CP-34). This arm is
    // only the provably-empty case — there is nothing to drop — and it exists so
    // the default kernel emits byte-identical code to the pre-C1.1 build
    // (`&[][..]`, no `Vec<InheritedHandle>` machinery). That makes the
    // `browser-runtime` kill switch's OFF equality an exact, measurable property
    // rather than a functional-equivalence argument.
    #[cfg(not(any(
        feature = "net-dial-cap",
        feature = "neo-test-arthur-compositor",
        feature = "neo-test-dw1-desktop-wm",
        feature = "neo-test-dw2-scene-policy",
        feature = "desktop-session",
        feature = "browser-runtime",
        feature = "cap-rooted-open",
        feature = "installer",
        feature = "atrium-compositor-g1",
        feature = "system-store",
        feature = "mount-authority",
        feature = "power-cap",
        feature = "neo-test-gpu-glass-slice6"
    )))]
    // Written as `&[][..]` (not `&[]`) to match the pre-C1.1 expression
    // exactly: the two forms are semantically identical, but they monomorphize
    // different index shims, and matching the original is what makes the
    // default kernel's SHA-256 bit-for-bit equal to the pre-slice build.
    let inherited: &[process::InheritedHandle] = &[][..];

    #[cfg(all(
        not(feature = "neo-test-gpu-glass-slice6"),
        any(
            feature = "net-dial-cap",
            feature = "neo-test-arthur-compositor",
            feature = "neo-test-dw1-desktop-wm",
            feature = "neo-test-dw2-scene-policy",
            feature = "desktop-session",
            feature = "browser-runtime",
            feature = "cap-rooted-open",
            feature = "installer",
            feature = "atrium-compositor-g1",
            feature = "system-store",
            feature = "mount-authority",
            feature = "power-cap"
        )
    ))]
    let inherited_vec: alloc::vec::Vec<process::InheritedHandle> = root_inherited_caps();
    #[cfg(feature = "neo-test-gpu-glass-slice6")]
    let inherited_vec: alloc::vec::Vec<process::InheritedHandle> = {
        let root_gpu_compute_caps = process::root_gpu_compute_test_caps()
            .expect("Slice-6 root GPU caps require the prepared graphics arena");
        root_gpu_compute_caps
            .iter()
            .map(|h| process::InheritedHandle {
                handle: h.clone(),
                parent: None,
            })
            .collect()
    };
    #[cfg(any(
        feature = "net-dial-cap",
        feature = "neo-test-arthur-compositor",
        feature = "neo-test-dw1-desktop-wm",
        feature = "neo-test-dw2-scene-policy",
        feature = "desktop-session",
        feature = "browser-runtime",
        feature = "cap-rooted-open",
        feature = "installer",
        feature = "atrium-compositor-g1",
        feature = "system-store",
        feature = "mount-authority",
        feature = "power-cap",
        feature = "neo-test-gpu-glass-slice6"
    ))]
    let inherited = &inherited_vec[..];
    let pid = process::spawn_user("neo-init", bytes, inherited)
        .expect("spawn_user(neo-init) failed — fatal at Phase 1.5 boot");
    serial_println!("[ kernel ] proc: neo-init spawned with pid={}", pid.0);

    // Yield + reap until neo-init has exited. The user process calls
    // sys_exit which routes through sched::exit; the reaper drains the
    // zombie queue and removes the Process from the registry.
    while process::any_user_process() {
        // OT-22: with the multiplex, sleep a tick per check instead of
        // yield-spinning, so an idle desktop leaves the BSP to its idle
        // thread (this loop was the last spinner on it). Off: as before.
        #[cfg(feature = "wait-multiplex")]
        sched::wait_any::sleep_ticks(1);
        #[cfg(not(feature = "wait-multiplex"))]
        sched::yield_now();
        sched::reap_and_drain();
    }
    // A drop deferred by the last pass's teardown (the #226 bounce).
    sched::reap_and_drain();
    serial_println!("[ kernel ] proc: neo-init reaped — back in kernel-only mode");
    let exit_code = process::take_spawned_exit_code(pid).unwrap_or_else(|| {
        serial_println!(
            "[ kernel ] proc: missing neo-init exit code after reap (pid={})",
            pid.0
        );
        255
    });
    serial_println!("[ kernel ] proc: neo-init exit code {}", exit_code);

    crate::drivers::net::virtio_net::service_all();
    mm::frame_audit("final-post-reap");
    let final_frames = mm::frames_in_use();
    debug_assert!(
        final_frames == baseline_frames,
        "frame leak: baseline={} final={} (delta=+{})",
        baseline_frames,
        final_frames,
        final_frames - baseline_frames,
    );
    serial_println!(
        "[ kernel ] frame: leak audit clean (baseline={} final={})",
        baseline_frames,
        final_frames
    );
    exit_code
}

fn initialize_framebuffer(required: bool) -> bool {
    let Some(fb_response) = FRAMEBUFFER_REQUEST.response() else {
        if required {
            panic!("limine: no framebuffer response (is the bootloader configured?)");
        }
        serial_println!("[ kernel ] framebuffer: no Limine response; framebuffer UI disabled");
        return false;
    };

    let Some(framebuffer) = fb_response.framebuffers().iter().next() else {
        if required {
            panic!("limine: framebuffer response had zero framebuffers");
        }
        serial_println!("[ kernel ] framebuffer: zero framebuffers; framebuffer UI disabled");
        return false;
    };

    serial_println!(
        "[ kernel ] framebuffer: {}x{} @ {} bpp, pitch {} addr 0x{:x}",
        framebuffer.width,
        framebuffer.height,
        framebuffer.bpp,
        framebuffer.pitch,
        framebuffer.address() as usize,
    );

    // SAFETY: Limine guarantees the framebuffer is valid for the lifetime of
    // the kernel, and the framebuffer module serializes all later writers.
    unsafe {
        drivers::fb::init(framebuffer);
    }
    #[cfg(feature = "framebuffer-console")]
    {
        drivers::fb::console::init();
        serial_println!(
            "[ kernel ] fb_console res={}x{} stride={}",
            framebuffer.width,
            framebuffer.height,
            framebuffer.pitch
        );
    }
    true
}

/// The kernel entry point. Limine jumps here after loading our ELF.
///
/// At this point:
///   - We are in long mode, ring 0.
///   - Interrupts are disabled.
///   - A flat identity-mapped page table is active (Limine sets this up).
///   - `cr3` points to Limine's page tables; we'll replace these in Phase 1.
/// `size-budget-probe`: one behaviour-free call, so
/// `cargo xtask size-budget --stability` can check that a change this small
/// moves the budgeted sizes by almost nothing. Never in an image.
#[cfg(feature = "size-budget-probe")]
#[inline(never)]
fn size_budget_probe(x: u64) -> u64 {
    x.rotate_left(7) ^ 3
}

#[unsafe(no_mangle)]
extern "C" fn _kernel_start() -> ! {
    // Bring up serial FIRST so any panic from the BASE_REVISION check
    // below has a place to land. If we asserted before init, a protocol
    // mismatch would panic into a black hole.
    drivers::console::serial::init();
    #[cfg(feature = "size-budget-probe")]
    core::hint::black_box(size_budget_probe(core::hint::black_box(1)));
    // L7.1: the kernel-entry stamp (a TSC read; nothing else is up yet).
    #[cfg(feature = "boot-time")]
    boot_time::stamp_entry();

    // Enable early CR0/CR4 CPU features before ANY further code runs. rustc
    // compiles for x86_64-unknown-none with SSE2 on by default; LLVM
    // happily vectorizes things like `slice::fill()`. If we hit a `movdqa`
    // before flipping CR4.OSFXSR, the CPU raises #UD and (with no IDT
    // installed yet) we triple-fault into oblivion. SMEP/SMAP are enabled
    // here too so every production boot gets the same policy as the tests.
    // SAFETY: single-threaded boot, we own the CPU.
    unsafe {
        arch::enable_sse();
    }

    // Phase 1.0: install the GDT (with TSS for IST) and IDT before anything
    // else might fault. After this returns, exceptions print a diagnostic
    // and halt instead of triple-faulting silently. Interrupts stay MASKED
    // until the explicit enable_interrupts() below.
    arch::init_traps();

    // Limine handshake. We requested revision 3 above. If the bootloader
    // serves it, `is_supported()` is true. If it served a different revision,
    // `actual_revision()` tells us which. Either way, we need at LEAST 1 so
    // pointers (framebuffer, modules, etc.) are higher-half mapped — at
    // revision 0 they're physical and dereferencing them from a higher-half
    // kernel will triple-fault.
    let supported = BASE_REVISION.is_supported();
    let actual = BASE_REVISION.actual_revision();
    if !supported {
        match actual {
            Some(r) if r >= 1 => { /* acceptable downgrade */ }
            Some(r) => panic!("limine: revision {r} too old (need >= 1 for HHDM pointers)"),
            None => panic!("limine: bootloader did not negotiate a base revision"),
        }
    }

    serial_println!();
    serial_println!("================================================");
    serial_println!("  LOGANSOS  v{}", env!("CARGO_PKG_VERSION"));
    // Which runtime checks this kernel carries: the product image is built
    // with core's UB precondition checks off (`-Zub-checks=no`) and
    // everything else on; scenario kernels keep all three.
    serial_println!(
        "  build {} | debug-assertions={} overflow-checks={} ub-checks={}",
        env!("NEO_BUILD_ID"),
        if cfg!(debug_assertions) { "on" } else { "off" },
        if cfg!(overflow_checks) { "on" } else { "off" },
        if cfg!(ub_checks) { "on" } else { "off" },
    );
    serial_println!("  Intelligence as a kernel primitive.");
    serial_println!("================================================");
    serial_println!();
    serial_println!(
        "[ kernel ] entered _kernel_start, long mode, ring 0 (limine rev: supported={}, actual={:?})",
        supported,
        actual
    );
    let (exec_phys, exec_virt) =
        executable_address().expect("limine: no executable address response");
    serial_println!(
        "[ kernel ] limine: executable phys=0x{:016x} virt=0x{:016x}",
        exec_phys,
        exec_virt
    );

    // Now that the IDT can field exceptions, allow maskable interrupts.
    arch::enable_interrupts();
    serial_println!("[ kernel ] interrupts: enabled");

    // Smoke-test the IDT: int3 should land in the breakpoint handler, print,
    // and return here. If this hangs or triple-faults, the IDT is broken.
    arch::breakpoint();
    serial_println!("[ kernel ] breakpoint smoke test: returned cleanly");

    // Phase 1.1: bring up the memory subsystem. After mm::init returns,
    // Box/Vec/String all work and the heap smoke test has passed.
    let hhdm_offset = HHDM_REQUEST
        .response()
        .expect("limine: no HHDM response (need base revision >= 0)")
        .offset;
    let exec_extent = mm::PhysExtent::new(
        exec_phys,
        exec_phys
            .checked_add(kernel_image_size())
            .expect("limine executable span overflow"),
    );
    let module_response = MODULE_REQUEST
        .response()
        .expect("limine: no module response (did limine.conf forget module_path?)");
    let modules = module_response.modules();
    // F7 (B1): name every staged module before the capacity check can panic.
    #[cfg(feature = "model-boot-module")]
    boot_modules::log_boot_modules(hhdm_offset);
    let mut module_extents = [mm::BootModuleExtent::EMPTY; MAX_BOOT_MODULE_EXTENTS];
    if modules.len() > module_extents.len() {
        panic!(
            "limine: module count {} exceeds boot HHDM extent capacity {}",
            modules.len(),
            module_extents.len()
        );
    }
    for (idx, module) in modules.iter().enumerate() {
        module_extents[idx] = mm::BootModuleExtent {
            extent: limine_file_data_extent(hhdm_offset, module),
        };
    }
    let boot_hhdm_requirements = mm::BootHhdmRequirements {
        executable: exec_extent,
        modules: &module_extents[..modules.len()],
    };
    let memmap = MEMMAP_REQUEST
        .response()
        .expect("limine: no memory map response")
        .entries();
    mm::init(hhdm_offset, memmap, &boot_hhdm_requirements);
    // L7.1: copy the UEFI map's firmware-RAM regions before anything can
    // reuse the bootloader's memory (the FPDT reader checks against them).
    #[cfg(feature = "boot-time")]
    boot_time::capture_firmware_regions();

    // Zenbook lane: from here every kernel log line is also on screen, so a
    // hang anywhere later in boot (APs, timer, storage) is photographable.
    #[cfg(feature = "fb-progress-console")]
    if initialize_framebuffer(false) {
        drivers::fb::progress::start();
    }

    // Phase 10.1 BUILD-10.1: initialize the kernel CSPRNG (entropy + ChaCha20 DRBG).
    // Runs after mm::init so serial logging works if RDRAND panics (D6 fail-closed).
    // Gated by `kernel-rng` feature; default-off ≡ today bit-for-bit.
    // In neo-test mode, uses a fixed seed for deterministic KATs.
    #[cfg(feature = "kernel-rng")]
    {
        rng::init();
        serial_println!("[ kernel ] rng: CSPRNG initialized (ChaCha20 DRBG, RDRAND + jitter)");
    }

    // Phase 7.5 BUILD-V (ISC-1): parse the ACPI DMAR table now that the HHDM and
    // frame allocator are live. Read-only; the `iommu` driver consumes the
    // cached result after the device probes. Gated by `vtd` (the kill-switch).
    #[cfg(feature = "vtd")]
    {
        let rsdp_phys = RSDP_REQUEST
            .response()
            .map(|r| r.address as u64)
            .unwrap_or(0);
        acpi::init(rsdp_phys);
    }

    // OT-90 P2: decode \_S5 and plan power-off and restart once, here, in
    // process context with the allocator live (THINK §3.2). The S5 and reset
    // paths later use only this cached plan. Gated by `power-control`.
    #[cfg(feature = "power-control")]
    acpi::power::init(
        RSDP_REQUEST
            .response()
            .map(|r| r.address as u64)
            .unwrap_or(0),
    );

    // OT-85: seed CLOCK_REALTIME's epoch from the CMOS RTC, once, read only
    // (the FADT's century index and "CMOS RTC Not Present" flag first). Any
    // failure keeps boot-relative realtime and logs one line; it never blocks
    // the boot. Gated by `rtc-epoch` (the kill-switch).
    #[cfg(feature = "rtc-epoch")]
    arch::x86_64::rtc::seed(
        RSDP_REQUEST
            .response()
            .map(|r| r.address as u64)
            .unwrap_or(0),
    );

    // Real AHCI/NVMe controllers can raise MSI/MSI-X as soon as the block
    // probe enables them. Initialize the BSP local APIC before block probing
    // so early storage interrupts can issue EOI without waiting for AP startup.
    arch::x86_64::smp::init_bsp_apic();

    // Phase 3 BUILD-1 storage probe point. PCI BAR mapping needs HHDM,
    // kernel page-table mutation, the frame allocator from mm::init, and a
    // BSP local APIC ready for any real-device MSI/MSI-X acknowledgement.
    // Keeping AP startup later still avoids cross-CPU scheduler ordering here.
    drivers::block::init();
    // The MT7601U bare-metal evidence sink must be durable before either
    // network stack or USB initialization can touch the dongle. This is a
    // sink-only bootstrap; framebuffer and generic test preparation retain
    // their normal ordering below.
    #[cfg(all(
        feature = "neo-test-bare-metal-mode",
        any(
            feature = "neo-test-mt7601u-scan-baremetal",
            feature = "neo-test-mt7601u-assoc-baremetal"
        )
    ))]
    test_harness::prepare_mt7601u_log_sink_before_io();
    drivers::net::init();
    #[cfg(feature = "usb-host")]
    drivers::usb::init();
    // The i8042 (PS/2) keyboard: after the USB stack has taken its
    // controllers, since firmware's USB legacy 8042 emulation ends at the
    // xHCI handoff. Gated by `ps2-keyboard` (the kill-switch).
    #[cfg(feature = "ps2-keyboard")]
    drivers::ps2::i8042::init(
        RSDP_REQUEST
            .response()
            .map(|r| r.address as u64)
            .unwrap_or(0),
    );
    // The Zenbook touchpad (I2C-HID): matched against the DSDT's own
    // description of it before anything is touched. Gated by
    // `i2c-hid-touchpad` (the kill-switch).
    #[cfg(feature = "i2c-hid-touchpad")]
    drivers::i2c_hid::touchpad::init(
        RSDP_REQUEST
            .response()
            .map(|r| r.address as u64)
            .unwrap_or(0),
    );

    // Phase 7.5 BUILD-V (ISC-3): bring up VT-d after the device probes.
    // Identity-domains every enumerated device (GPA==HPA — no regression) and
    // enables translation; connectivity devices are narrowed to fence domains
    // on demand (the arena lane / the (c2) proof). Gated by `vtd`; fails safe
    // (DMA stays physical) if the platform doesn't expose VT-d.
    #[cfg(feature = "vtd")]
    drivers::iommu::init();

    // The RTL8125 (rung 1): after VT-d, so its DMA-containment decision sees
    // whether the device can be fenced; with no supported IOMMU it binds,
    // reports MAC and link, and leaves bus mastering off unless the separate
    // `rtl8125-dma-without-iommu` opt-in is built in. Off ⇒ not compiled.
    #[cfg(feature = "rtl8125")]
    drivers::net::rtl8125::init();

    // I-14(c) for the iGPU (Slice-4 AAG bounce, 2026-08-01). The Intel accel
    // driver is a real bus-mastering DMA master, so its containment must be a
    // hardware fact BEFORE the probe runs — `accel::init()` is reached later
    // via `ai::init()`. Reserve the graphics working-set arena and move the
    // iGPU requester into the narrowed graphics domain here; the probe refuses
    // bus mastering if this did not succeed, leaving the device decode-only.
    // `neo-test-accel-intel-blur-silicon` is the REFUSAL scenario (DECISION
    // 2026-08-01 §1): it deliberately boots with containment disarmed so the
    // I-14(c) guard's refusal path is exercised on real silicon rather than
    // merely believed in. Every other configuration arms containment here.
    #[cfg(all(
        feature = "accel-intel",
        feature = "vtd",
        not(feature = "neo-test-accel-intel-blur-silicon")
    ))]
    drivers::graphics_arena::arm_boot_containment();

    #[cfg(feature = "neo-test")]
    {
        initialize_framebuffer(false);
        test_harness::prepare_bare_metal_mode();
    }

    // Phase 3 BUILD-1 VFS/testfs root. This remains an in-memory testfs
    // until BUILD-2 introduces a real NeoFS root.
    fs::init();
    // Phase 7.5 CB-3 (ISC-4): the synthetic /net interface namespace (interface
    // capabilities). Inert unless `interface-identity` is on.
    fs::mount_net_interface_fs();
    #[cfg(feature = "model-boot-module")]
    boot_modules::mount_boot_fs();
    ai::init();

    // Phase 2 BUILD-3: start APs after memory is online but before the BSP
    // scheduler exists. APs construct their own per-CPU schedulers and idle in
    // local reaper loops; no work is routed to them yet.
    arch::x86_64::smp::bring_up_aps();

    // Phase 1.2: bring up the cooperative scheduler and run the ping/pong
    // smoke test. After smoke_test returns, two kernel threads have
    // demonstrably alternated via yield_now().
    // CP-69: the independent clocks the LAPIC timer is calibrated against.
    #[cfg(feature = "lapic-timer")]
    arch::x86_64::refclock::discover(
        RSDP_REQUEST
            .response()
            .map(|r| r.address as u64)
            .unwrap_or(0),
    );
    sched::init_bsp();
    sched::smoke_test();

    // Zenbook USB input lane: from here a kernel thread re-reads the USB
    // ports and enumerates devices that connect after the boot scan.
    #[cfg(feature = "usb-hotplug")]
    drivers::usb::start_port_service();
    // The touchpad's service thread (drains the report on each interrupt).
    #[cfg(feature = "i2c-hid-touchpad")]
    drivers::i2c_hid::touchpad::start_service();
    // OT-96: wire receive after bring-up, once every driver's bring-up loop
    // (the RTL8125's DHCP loop ran in `rtl8125::init` above) is done.
    #[cfg(feature = "wire-rx")]
    net::rx::start();
    // OT-95: the periodic net tick, after every driver's bring-up loop (the
    // RTL8125's DHCP loop ran in `rtl8125::init` above).
    #[cfg(feature = "net-tick")]
    net::periodic::start();

    // OT-78 test-only catch-all discriminator: hold an existing `/neofs`
    // mount busy so the boot mount selects the second (valid) disk and then
    // fails through its ordinary mount-table error path. The harness below
    // asserts that boot still reaches user-independent scenario execution.
    #[cfg(feature = "neo-test-boot-neofs-unexpected-error")]
    fs::debug_prepare_boot_neofs_busy_mount()
        .expect("prepare boot-neofs unexpected-error scenario");

    // `boot-neofs` mounts through the block layer, and virtio-blk can park/yield
    // while waiting for completion. Keep the product mount before userland, but
    // after the BSP scheduler exists.
    #[cfg(feature = "boot-neofs")]
    fs::mount_boot_neofs_from_device0_readwrite();

    // OT-34 (Milestone-A): enable threaded forward matmul now that the APs are up
    // and busy-yielding in `run_ap_bootstrap` (each polling `ap_poll`) and the BSP
    // scheduler + shootdown-readiness are installed. This raises the per-token
    // forward matmul from single-threaded scalar to a deterministic
    // column-partition fork-join across the online cores — a faster Arthur without
    // changing a single output bit (proven by `neo-test-threaded-matmul-parity`:
    // byte + greedy-argmax token identity at thread_count 1/2/4 on real APs, plus
    // an armed negative control; end-to-end real-brain token parity is the
    // bare-metal close). Kill-switch intact:
    // feature-off removes this call and `par_matmul` entirely; a single-CPU boot
    // clamps to 1 (single-threaded); a caller/KAT may still override the count.
    // Determinism is independent of AP participation — `par_matmul`'s steal-back
    // gives the BSP forward progress even if every AP is momentarily absent
    // (CP-20 guard), so raising the count can slow but never hang the forward.
    #[cfg(feature = "threaded-matmul")]
    crate::ai::par_matmul::set_thread_count(arch::x86_64::smp::online_cpu_count());

    // Arthur latency (NOTE fix 2): publish the AVX2 census now that every
    // online CPU has reported its own check from its `enable_sse` (after its
    // XCR0 setup). "Yes" only if all reported and none said no; otherwise "no"
    // (a veto) or undecided (each call keeps its own local check).
    #[cfg(any(feature = "phase9-quantized-matmul", feature = "phase9-real-quant"))]
    {
        let online = arch::x86_64::smp::online_cpu_count() as u32;
        let decision = crate::ai::ops::qk_matmul::publish_avx2_decision(online);
        serial_println!(
            "[ kernel ] ai: matmul avx2={:?} (census: {}/{} cpus reported)",
            decision,
            crate::ai::ops::qk_matmul::AVX2_CENSUS.reported(),
            online
        );
    }

    // Arthur perf lane (OT-7): calibrate the TSC→time conversion against the 100 Hz
    // PIT now — interrupts are enabled (line 442) so the PIT is ticking, and this
    // is the safe (IRQ-on) place to do it (the forward runs IRQ-off). Idempotent;
    // the showcase reports per-forward-ms from it, replacing Milestone-A's ms=0.
    #[cfg(feature = "forward-perf")]
    crate::ai::forward_perf::calibrate_tsc_hz();

    // Zenbook ask-freeze: build the boot model now, before userland, with
    // interrupts on and on the guarded inference stack (`boot_modules::preload`).
    // The tick discriminator calls it itself, beside a co-runner.
    #[cfg(all(
        feature = "boot-model-preload",
        not(feature = "neo-test-boot-model-ticks")
    ))]
    {
        boot_modules::preload();
        // L7.1: the model Arthur answers with is built (an image without
        // one, or whose build failed, has no model-ready time).
        #[cfg(feature = "boot-time")]
        if boot_modules::any_model_built() {
            boot_time::stamp_model_ready();
        }
    }

    #[cfg(feature = "neo-test")]
    test_harness::run_selected_scenario(spawn_neo_init);

    // Desktop S1: the production session boot. The framebuffer comes up
    // BEFORE neo-init, the Software present backend is selected, and the
    // scarce scanout authority is armed (only the GpuSubmit holder — neo-init,
    // the compositor/WM — may reach raw scanout). neo-init then stays resident
    // as the session compositor; `spawn_neo_init` only returns if the session
    // ends, in which case the classic splash below is the fallback screen.
    // Feature-off ⇒ none of this is compiled (the kill-switch).
    #[cfg(all(feature = "desktop-session", not(feature = "neo-test")))]
    {
        // Zenbook USB input lane: the USB summary is the last thing the boot
        // console shows. With `boot-evidence-holds` (a bring-up image) it
        // stays up (5 s for a device to bind, then up to 30 s) when no
        // keyboard or mouse is bound, so it can be photographed; with one
        // bound, a 6 s input check counts the reports that arrive. Without it
        // (image A) the same summary is printed (serial, and the boot console
        // until the desktop replaces it) and the boot does not wait.
        #[cfg(all(
            feature = "boot-evidence-holds",
            feature = "fb-progress-console",
            feature = "xhci-silicon"
        ))]
        drivers::usb::input_evidence_hold_for(500, 3000, 600);
        #[cfg(all(
            not(feature = "boot-evidence-holds"),
            feature = "fb-progress-console",
            feature = "xhci-silicon"
        ))]
        drivers::usb::input_evidence_summary();
        // The RTL8125's boot lines (bind, DHCP, gateway, or `absent`) again,
        // just above the touchpad check, whose hold keeps them on screen.
        #[cfg(all(feature = "rtl8125", feature = "fb-progress-console"))]
        drivers::net::rtl8125::evidence_summary();
        // The touchpad's check: the last thing on the boot console. Without
        // `boot-evidence-holds`, its lines only (no 6 s check, no 30 s hold).
        #[cfg(all(
            feature = "boot-evidence-holds",
            feature = "i2c-hid-touchpad",
            feature = "fb-progress-console"
        ))]
        drivers::i2c_hid::touchpad::evidence_check(600, 3000);
        #[cfg(all(
            not(feature = "boot-evidence-holds"),
            feature = "i2c-hid-touchpad",
            feature = "fb-progress-console"
        ))]
        drivers::i2c_hid::touchpad::evidence_lines();
        // L7.1: the desktop-start stamp and the boot console's last line.
        #[cfg(feature = "boot-time")]
        boot_time::stamp_desktop(
            RSDP_REQUEST
                .response()
                .map(|r| r.address as u64)
                .unwrap_or(0),
        );
        // The compositor owns the screen from here: stop the boot mirror.
        #[cfg(feature = "fb-progress-console")]
        drivers::fb::progress::stop();
        initialize_framebuffer(true);
        // KERNEL_READY (panic-to-framebuffer) deliberately stays unset while
        // the session runs: the compositor owns scanout, and a panic painting
        // over it could spin on the fb lock a present holds. Panics still
        // reach serial; the flag is set below once the session has ended.
        atrium::present_backend::set_mode(atrium::present_backend::PresentMode::Software);
        atrium::present_backend::set_gpu_submit_enforce(true);
        serial_println!(
            "[ kernel ] desktop-session: framebuffer={}x{} scanout_gate=armed compositor=neo-init",
            drivers::fb::dimensions().0,
            drivers::fb::dimensions().1
        );
    }

    #[cfg(not(feature = "neo-test"))]
    {
        // Phase 1.4: load neo-init from the Limine module list, ELF-parse it,
        // create a user Process with its own page table, switch to ring 3.
        let neo_init_exit_code = spawn_neo_init();
        if neo_init_exit_code != 0 {
            serial_println!(
                "[ kernel ] proc: neo-init completed with nonzero exit code {}",
                neo_init_exit_code
            );
        }

        initialize_framebuffer(true);

        // We're past the dangerous part. If we panic from here on, the panic
        // handler can render to the framebuffer.
        KERNEL_READY.store(true, Ordering::Release);

        // Render the boot sequence. This is the iconic LogansOS moment.
        init::boot_frame::render_boot_sequence();

        serial_println!("[ kernel ] phase 0 boot complete");
        serial_println!("[ kernel ] no scheduler yet — halting forever");

        // Phase 0 has no scheduler, no userspace, no work to do. Park the CPU.
        // In Phase 1 this becomes the idle loop that yields to the scheduler.
        arch::halt_forever();
    }
}

/// neo-init's root capability table, in its index order (CP-34: each
/// authority pushes only under its own feature, so no composition drops one).
/// Janus's #8 review, item 1a: `power-cap` is in this gate and its callers',
/// so a kernel whose only root authority is `power-cap` still installs the
/// PowerControl cap; `ot90_cap` reads its cap from this table.
// Not `vec![...]`: every push below is cfg-gated, which is the entire point
// of the compositional assembly (CP-34). A literal initializer cannot
// express "include this element only when a feature is on". The allow sits
// on the BLOCK, not the `let` — the lint's span covers creation plus the
// pushes, so a statement-level allow does not suppress it.
#[cfg(all(
    not(feature = "neo-test-gpu-glass-slice6"),
    any(
        feature = "net-dial-cap",
        feature = "neo-test-arthur-compositor",
        feature = "neo-test-dw1-desktop-wm",
        feature = "neo-test-dw2-scene-policy",
        feature = "desktop-session",
        feature = "browser-runtime",
        feature = "cap-rooted-open",
        feature = "installer",
        feature = "atrium-compositor-g1",
        feature = "system-store",
        feature = "mount-authority",
        feature = "power-cap"
    )
))]
#[allow(clippy::vec_init_then_push)]
pub(crate) fn root_inherited_caps() -> alloc::vec::Vec<sched::process::InheritedHandle> {
    use sched::process;
    #[allow(unused_mut)]
    let mut caps: alloc::vec::Vec<process::InheritedHandle> = alloc::vec::Vec::new();
    #[cfg(feature = "net-dial-cap")]
    caps.push(process::InheritedHandle {
        handle: process::root_net_dial_cap(),
        parent: None,
    });
    #[cfg(any(
        feature = "neo-test-arthur-compositor",
        feature = "neo-test-dw1-desktop-wm",
        feature = "neo-test-dw2-scene-policy",
        feature = "desktop-session"
    ))]
    caps.push(process::InheritedHandle {
        handle: process::root_gpu_submit_cap(),
        parent: None,
    });
    // DI-1a: the session root's non-delegable input-system authority.
    #[cfg(all(feature = "desktop-session", feature = "atrium-pointer-forward"))]
    caps.push(process::InheritedHandle {
        handle: process::root_input_system_cap(),
        parent: None,
    });
    #[cfg(feature = "browser-runtime")]
    caps.push(process::InheritedHandle {
        handle: process::root_browser_runtime_cap(),
        parent: None,
    });
    // OT-87 R4.4: the installer authority, for neo-init to hand to the
    // shell and the installer screen.
    #[cfg(feature = "installer")]
    caps.push(process::InheritedHandle {
        handle: process::root_installer_cap(),
        parent: None,
    });
    // OT-66 K1a: last, so the root caps above keep their indices.
    #[cfg(feature = "cap-rooted-open")]
    caps.push(process::InheritedHandle {
        handle: process::root_dir_cap(),
        parent: None,
    });
    // Moving the keyboard route takes the input-system authority (L2
    // pre-merge AAG, F1), so the root holds it wherever the focus
    // syscalls exist, not only with the pointer route. After the caps
    // above, which keep their indices.
    #[cfg(all(
        feature = "atrium-compositor-g1",
        not(all(feature = "desktop-session", feature = "atrium-pointer-forward"))
    ))]
    caps.push(process::InheritedHandle {
        handle: process::root_input_system_cap(),
        parent: None,
    });
    // OT-66 L1: the sign-in record's store, neo-init's alone. Last, so
    // every cap above keeps its index.
    #[cfg(feature = "system-store")]
    caps.push(process::InheritedHandle {
        handle: process::root_system_store_cap(),
        parent: None,
    });
    // Dexter #187 item 3a: the mount authority, neo-init's alone. After
    // the system store, so every cap above keeps its index.
    #[cfg(feature = "mount-authority")]
    caps.push(process::InheritedHandle {
        handle: process::root_mount_authority_cap(),
        parent: None,
    });
    // OT-90 P4: Shut down and Restart, neo-init's alone. Last, after the
    // mount authority, so every cap above keeps its index.
    #[cfg(feature = "power-cap")]
    caps.push(process::InheritedHandle {
        handle: process::root_power_control_cap(),
        parent: None,
    });
    caps
}
