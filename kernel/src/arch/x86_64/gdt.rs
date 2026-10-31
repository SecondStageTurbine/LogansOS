// SPDX-License-Identifier: MPL-2.0
use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::Ordering;

use crate::serial_println;
use spin::Once;
use x86_64::VirtAddr;
use x86_64::instructions::segmentation::{CS, DS, ES, FS, GS, SS, Segment};
use x86_64::instructions::tables::load_tss;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;

use crate::arch::x86_64::per_cpu::{MAX_CPUS, PER_CPU, current_per_cpu};

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;
pub const NMI_IST_INDEX: u16 = 1;
pub const TIMER_IST_INDEX: u16 = 2;

const STACK_SIZE: usize = 20 * 1024;
/// Slot 3 (selector 0x18/0x1B) is a null descriptor on purpose. It used to be
/// a DPL3 32-bit code segment, which let ring 3 `retfq` into compatibility
/// mode (and from there run `syscall` through the unprogrammed CSTAR, or
/// `bound`). Nothing runs 32-bit code: userland is x86-64 only and every
/// return to ring 3 is IRETQ to the 64-bit code segment. The slot stays so the
/// user selectors (0x23 data, 0x2B code) and STAR's SYSRET arithmetic
/// (user base = 0x1B, data = base+8, code = base+16) are unchanged (CP-48).
const COMPAT_CODE_HOLE_RAW: u64 = 0;

#[repr(align(16))]
struct IstStack(UnsafeCell<[u8; STACK_SIZE]>);

unsafe impl Sync for IstStack {}

impl IstStack {
    #[allow(clippy::declare_interior_mutable_const)] // per-CPU IST stack template — UnsafeCell is the point
    const EMPTY: Self = Self(UnsafeCell::new([0; STACK_SIZE]));

    fn top(&self) -> VirtAddr {
        VirtAddr::from_ptr(self.0.get().cast::<u8>()) + STACK_SIZE as u64
    }
}

struct PerCpuIstStacks {
    double_fault: IstStack,
    nmi: IstStack,
    timer: IstStack,
}

impl PerCpuIstStacks {
    #[allow(clippy::declare_interior_mutable_const)] // per-CPU IST stack template — UnsafeCell is the point
    const EMPTY: Self = Self {
        double_fault: IstStack::EMPTY,
        nmi: IstStack::EMPTY,
        timer: IstStack::EMPTY,
    };
}

struct TssSlot(UnsafeCell<MaybeUninit<TaskStateSegment>>);

unsafe impl Sync for TssSlot {}

impl TssSlot {
    #[allow(clippy::declare_interior_mutable_const)] // per-CPU TSS slot template — UnsafeCell is the point
    const EMPTY: Self = Self(UnsafeCell::new(MaybeUninit::uninit()));
}

struct PerCpuGdt {
    gdt: GlobalDescriptorTable,
    selectors: Selectors,
}

struct Selectors {
    kernel_code: SegmentSelector,
    kernel_data: SegmentSelector,
    user_data: SegmentSelector,
    user_code_64: SegmentSelector,
    tss: SegmentSelector,
}

/// Per-CPU GDT array. Each AP loads its own entry via `lgdt` during init.
///
/// Per-CPU GDT array. Each CPU's GDT embeds that CPU's TSS descriptor; the
/// TSS itself lives in kernel-eternal static storage because trap init runs
/// before the heap is available.
static GDTS: [Once<PerCpuGdt>; MAX_CPUS] = [const { Once::new() }; MAX_CPUS];

/// Static per-CPU IST stack storage.
///
/// Trap init currently runs before `mm::init()`, so sub-commit 1a cannot use
/// heap allocation here without changing `main.rs`. Each CPU still owns a
/// distinct TSS and IST stack set; the storage is kernel-eternal either way.
static IST_STACKS: [PerCpuIstStacks; MAX_CPUS] = [const { PerCpuIstStacks::EMPTY }; MAX_CPUS];
static TSS_STORAGE: [TssSlot; MAX_CPUS] = [const { TssSlot::EMPTY }; MAX_CPUS];

fn initialize_per_cpu_tss(cpu_id: u32) -> *mut TaskStateSegment {
    let idx = cpu_id as usize;
    let mut tss = TaskStateSegment::new();

    let stacks = &IST_STACKS[idx];
    tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = stacks.double_fault.top();
    tss.interrupt_stack_table[NMI_IST_INDEX as usize] = stacks.nmi.top();
    tss.interrupt_stack_table[TIMER_IST_INDEX as usize] = stacks.timer.top();

    let slot = TSS_STORAGE[idx].0.get();
    unsafe {
        (*slot).write(tss);
        (*slot).as_mut_ptr()
    }
}

pub fn init_bsp() {
    init_for_cpu(0);
}

pub fn init_ap(cpu_id: u32) {
    init_for_cpu(cpu_id);
}

fn init_for_cpu(cpu_id: u32) {
    let idx = cpu_id as usize;
    assert!(idx < MAX_CPUS, "gdt: cpu_id exceeds MAX_CPUS");

    let tss_ptr = initialize_per_cpu_tss(cpu_id);

    PER_CPU[idx].tss_ptr.store(tss_ptr, Ordering::Release);

    let tss_ref: &'static TaskStateSegment = unsafe { &*tss_ptr };

    let mut gdt = GlobalDescriptorTable::new();
    let kernel_code = gdt.append(Descriptor::kernel_code_segment());
    let kernel_data = gdt.append(Descriptor::kernel_data_segment());
    gdt.append(Descriptor::UserSegment(COMPAT_CODE_HOLE_RAW));
    let user_data = gdt.append(Descriptor::user_data_segment());
    let user_code_64 = gdt.append(Descriptor::user_code_segment());
    let tss_sel = gdt.append(Descriptor::tss_segment(tss_ref));

    let selectors = Selectors {
        kernel_code,
        kernel_data,
        user_data,
        user_code_64,
        tss: tss_sel,
    };

    GDTS[idx].call_once(|| PerCpuGdt { gdt, selectors });

    let pcg = GDTS[idx].get().expect("gdt: cpu not initialized");
    pcg.gdt.load();

    unsafe {
        CS::set_reg(pcg.selectors.kernel_code);
        DS::set_reg(pcg.selectors.kernel_data);
        ES::set_reg(pcg.selectors.kernel_data);
        FS::set_reg(pcg.selectors.kernel_data);
        GS::set_reg(pcg.selectors.kernel_data);
        SS::set_reg(pcg.selectors.kernel_data);
        load_tss(pcg.selectors.tss);
        // A null selector marks LDTR invalid, so no TI=1 selector resolves.
        // After reset LDTR is a usable table at linear 0 (limit 0xFFFF), and
        // nothing guarantees the firmware or bootloader reloaded it: a far jump
        // through a TI=1 selector would then use a descriptor from page 0 of
        // the current address space, which ring 3 may map (CP-48).
        core::arch::asm!("lldt {0:x}", in(reg) 0u16, options(nostack, preserves_flags));
    }

    serial_println!(
        "[ kernel ] gdt: cpu={} loaded (tss_sel={:#x})",
        cpu_id,
        pcg.selectors.tss.0
    );
}

fn bsp_selectors() -> &'static Selectors {
    &GDTS[0].get().expect("gdt: BSP not initialized").selectors
}

pub fn kernel_code_selector() -> SegmentSelector {
    bsp_selectors().kernel_code
}

pub fn kernel_data_selector() -> SegmentSelector {
    bsp_selectors().kernel_data
}

pub fn user_code_64_selector() -> SegmentSelector {
    bsp_selectors().user_code_64
}

pub fn user_data_selector() -> SegmentSelector {
    bsp_selectors().user_data
}

#[allow(dead_code)]
pub fn tss_selector_for_cpu(cpu_id: u32) -> SegmentSelector {
    GDTS[cpu_id as usize]
        .get()
        .expect("gdt: cpu not initialized")
        .selectors
        .tss
}

/// Raw selectors used when constructing the initial ring-3 IRETQ frame.
pub const USER_CODE_SEL_RAW: u64 = 0x2B;
pub const USER_DATA_SEL_RAW: u64 = 0x23;

/// User-side RFLAGS for first ring-3 entry: IF (bit 9) plus the reserved
/// must-be-1 bit (bit 1).
pub const USER_RFLAGS_RAW: u64 = (1 << 9) | (1 << 1);

pub fn set_kernel_rsp(rsp: u64) {
    let tss_ptr = current_per_cpu().tss_ptr.load(Ordering::Acquire);
    debug_assert!(!tss_ptr.is_null(), "set_kernel_rsp: TSS pointer not set");
    unsafe {
        (*tss_ptr).privilege_stack_table[0] = VirtAddr::new(rsp);
    }
}
