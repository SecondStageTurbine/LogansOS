// SPDX-License-Identifier: MPL-2.0
//! Cross-CPU TLB shootdown machinery.
//!
//! The sender API takes an explicit CPU target set; SC-3C production Pmap
//! call sites derive and pass `other_cpus_mask()`.

#![cfg_attr(feature = "neo-test-missing-shootdown-negative", allow(dead_code))]

use core::arch::asm;
use core::ptr;
use core::sync::atomic::{AtomicU32, Ordering};

use ::x86_64::VirtAddr;

use crate::arch::x86_64::{self as arch_x86_64, apic, per_cpu};
use crate::sync::SwitchMutex;

const PAGE_SIZE: u64 = 4096;
pub const INVLPG_THRESHOLD_PAGES: u64 = 33;
const SHOOTDOWN_RETRY_AFTER_MS: u64 = 1;
const SHOOTDOWN_FINAL_TIMEOUT_MS: u64 = 50;
const FALLBACK_TSC_CYCLES_PER_MS: u64 = 10_000_000;

// The IPI handler never takes this lock. Keep sender serialization
// interruptible so a CPU waiting behind another sender can still ACK that
// sender's shootdown IPI. An IrqSafeMutex here creates an ABBA deadlock:
// holder waits for the target's ACK while the target spins on this lock with
// local interrupts disabled.
static SHOOTDOWN_LOCK: SwitchMutex<()> = SwitchMutex::new(());

/// Temporarily removes the current CPU from remote shootdown target sets.
/// `resume` performs a full local flush before making the CPU targetable again.
pub struct LocalShootdownQuiesce {
    cpu_id: u32,
    resumed: bool,
}

impl LocalShootdownQuiesce {
    pub fn resume(mut self) {
        assert!(
            !arch_x86_64::interrupts_enabled(),
            "shootdown quiesce must resume with IF=0"
        );
        let barrier = acquire_shootdown_lock();
        invalidate_local(None);
        let was_ready = per_cpu::set_current_tlb_shootdown_ready(true);
        debug_assert!(!was_ready, "shootdown quiesce resumed twice");
        drop(barrier);
        arch_x86_64::enable_interrupts();
        self.resumed = true;
    }
}

impl Drop for LocalShootdownQuiesce {
    fn drop(&mut self) {
        debug_assert!(
            self.resumed,
            "CPU {} dropped shootdown quiesce without resume",
            self.cpu_id
        );
    }
}

/// Stop targeting this CPU without exposing a ready-but-unserviceable window.
/// Taking the sender lock first drains any sender that sampled the old ready
/// bit. IF is then masked and readiness withdrawn while no sender can race the
/// transition. The returned guard leaves IF=0 until `resume`.
pub fn quiesce_local_cpu() -> LocalShootdownQuiesce {
    let cpu_id = per_cpu::current_cpu_id();
    let barrier = acquire_shootdown_lock();
    arch_x86_64::disable_interrupts();
    let was_ready = per_cpu::set_current_tlb_shootdown_ready(false);
    assert!(was_ready, "CPU {} was not shootdown-ready", cpu_id);
    drop(barrier);
    LocalShootdownQuiesce {
        cpu_id,
        resumed: false,
    }
}

/// OT-90 P3b, K4 (review #6 item 1, CP-64): close TLB shootdowns for good
/// before the shutdown CPU stops servicing IPIs. Taking the sender lock
/// (interruptibly, as every sender does) waits out any sender in flight, so
/// this CPU acknowledges its request first; the lock is then never released,
/// so no sender starts again (a CPU that tries waits with interrupts on and
/// takes the STOP IPI). Interrupts go off and this CPU leaves the ready set
/// while no sender can race the change. Returns with IF=0, for good.
#[cfg(feature = "power-stop-cpus")]
pub fn close_for_stop() {
    let barrier = acquire_shootdown_lock();
    arch_x86_64::disable_interrupts();
    per_cpu::set_current_tlb_shootdown_ready(false);
    core::mem::forget(barrier);
}

#[cfg(feature = "neo-test-swapgs-irq-ring3")]
static LOCK_PROBE_HELD: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "neo-test-swapgs-irq-ring3")]
static LOCK_PROBE_WAITER_CPU: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "neo-test-swapgs-irq-ring3")]
static LOCK_PROBE_DONE: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "neo-test-swapgs-irq-ring3")]
static LOCK_PROBE_ERROR: AtomicU32 = AtomicU32::new(0);

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct CpuMask(u64);

impl CpuMask {
    pub const fn empty() -> Self {
        Self(0)
    }

    #[allow(dead_code)] // Used by q7c timeout instrumentation builds.
    pub fn from_cpu(cpu_id: u32) -> Self {
        let mut mask = Self::empty();
        mask.insert(cpu_id);
        mask
    }

    pub fn bits(self) -> u64 {
        self.0
    }

    #[allow(dead_code)] // Used by q7c synthetic instrumentation builds.
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn count(self) -> u32 {
        self.0.count_ones()
    }

    pub fn contains(self, cpu_id: u32) -> bool {
        cpu_id < per_cpu::MAX_CPUS as u32 && (self.0 & (1u64 << cpu_id)) != 0
    }

    pub fn insert(&mut self, cpu_id: u32) {
        if cpu_id < per_cpu::MAX_CPUS as u32 {
            self.0 |= 1u64 << cpu_id;
        }
    }

    pub fn without_cpu(self, cpu_id: u32) -> Self {
        if cpu_id >= per_cpu::MAX_CPUS as u32 {
            return self;
        }
        Self(self.0 & !(1u64 << cpu_id))
    }

    pub fn intersection(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }

    pub fn iter(self) -> CpuMaskIter {
        CpuMaskIter { bits: self.0 }
    }
}

pub struct CpuMaskIter {
    bits: u64,
}

impl Iterator for CpuMaskIter {
    type Item = u32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.bits == 0 {
            return None;
        }
        let cpu = self.bits.trailing_zeros();
        self.bits &= self.bits - 1;
        Some(cpu)
    }
}

/// Payload published through each target CPU's per-CPU shootdown slot.
///
/// Phase 2+ lazy-TLB note: page-table-teardown exception
///
/// SC-3C does not implement lazy-TLB (D6: out of scope). If Phase 2+ work
/// introduces lazy-TLB optimization, the Linux pattern requires an explicit
/// exception: the lazy filter MUST be bypassed unconditionally when the
/// shootdown is for page-table teardown (`freed_tables=true` in Linux). A CPU
/// with stale upper-level paging caches can fault into freed memory after the
/// teardown completes. If NeoPlutarch lands lazy-TLB later, the same exception
/// must be encoded in this API.
pub struct ShootdownPayload {
    range: Option<(VirtAddr, VirtAddr)>,
    targets: CpuMask,
    pending: AtomicU32,
}

impl ShootdownPayload {
    fn new(range: Option<(VirtAddr, VirtAddr)>, targets: CpuMask) -> Self {
        Self {
            range,
            targets,
            pending: AtomicU32::new(targets.count()),
        }
    }
}

/// Invalidate `range` on the target CPU set.
///
/// `range == None` requests a full local TLB flush on each target.
/// `range == Some((start, end))` requests INVLPG for each page in the
/// half-open range if the page count is <= 33, per SC-3C D3's Linux-measured
/// threshold; larger ranges fall back to full local flushes.
///
/// `targets` is an explicit CPU bitmask. SC-3C production call sites pass
/// `other_cpus_mask()` per D4/T1 broadcast. Future Phase 2+ T2 work may pass a
/// narrower target set derived from per-mm CPU state.
///
/// Self-invalidation runs synchronously after IPI dispatch and before the ACK
/// wait loop, matching SC-3C D9's FreeBSD `curcpu_cb` sequencing pattern.
/// The sender returns when all targets ACK. Targets that have not consumed the
/// pending slot after the first 1 ms window are re-poked once; the sender only
/// panics after the larger final deadline still leaves ACKs pending.
///
/// Phase 2+ T2 migration note: per-mm cpumask design choice
///
/// SC-3C ships T1 broadcast. Phase 2+ T2 work will derive a narrower target
/// set from per-mm cpumask state, reducing IPI count to CPUs that have loaded
/// the mm's CR3. That design must choose between a strictly-accurate mask
/// (remove-on-switch-out plus add-on-switch-in) and a loosely-accurate mask
/// (Linux's add-only switch-in model, with stale bits scrubbed lazily by IPI
/// receivers). Linux's choice favors the hotter context-switch path over the
/// colder IPI path. NeoPlutarch's inference workloads may shift that tradeoff;
/// an ADR-tier decision is required before T2 lands.
///
/// Phase 2+ INVLPGB/RAR note: global-ASID promotion mechanism
///
/// SC-3C's broadcast model and N=33 INVLPG threshold match the conservative
/// single-target pattern of pre-INVLPGB Linux. AMD INVLPGB in Linux 6.15 adds
/// hardware-broadcast TLB invalidation for global-ASID-tagged mms. Linux's
/// `consider_global_asid` promotion mechanism uses a 1-in-32 statistical
/// sampling gate plus a rising floor as the global-ASID bitmap fills, not a
/// deterministic 3-CPU threshold. The INVLPGB integration function name is
/// `broadcast_tlb_flush`.
pub fn shootdown(range: Option<(VirtAddr, VirtAddr)>, targets: CpuMask) {
    let _guard = acquire_shootdown_lock();
    shootdown_while_locked(range, targets);
}

fn shootdown_while_locked(range: Option<(VirtAddr, VirtAddr)>, targets: CpuMask) {
    let self_cpu = per_cpu::current_cpu_id();
    let targets = targets
        .intersection(shootdown_ready_cpus_mask())
        .without_cpu(self_cpu);
    let payload = ShootdownPayload::new(range, targets);
    let payload_ptr = &payload as *const ShootdownPayload as *mut ShootdownPayload;

    publish_payload_to_targets(payload_ptr, targets);
    for cpu_id in targets.iter() {
        #[cfg(feature = "neo-test-instrumentation")]
        record_initial_ipi_send(cpu_id);
        apic::send_tlb_shootdown_ipi(cpu_id);
    }

    #[cfg(feature = "neo-test-instrumentation")]
    record_self_invalidation();
    invalidate_local(range);

    wait_for_acks_or_panic(&payload, targets);
}

fn acquire_shootdown_lock() -> crate::sync::switch_mutex::SwitchMutexGuard<'static, ()> {
    if let Some(guard) = SHOOTDOWN_LOCK.try_lock() {
        return guard;
    }

    #[cfg(feature = "neo-test-swapgs-irq-ring3")]
    LOCK_PROBE_WAITER_CPU.store(per_cpu::current_cpu_id() + 1, Ordering::Release);

    // The wait must be interruptible even when the caller arrived with IF=0:
    // otherwise this CPU cannot ACK the holder's shootdown. Restore the
    // caller's state immediately after acquisition, before entering the
    // serialized sender body.
    let interrupts_were_enabled = arch_x86_64::interrupts_enabled();
    if !interrupts_were_enabled {
        arch_x86_64::enable_interrupts();
    }
    let guard = SHOOTDOWN_LOCK.lock();
    if !interrupts_were_enabled {
        arch_x86_64::disable_interrupts();
    }
    guard
}

#[cfg(feature = "neo-test-swapgs-irq-ring3")]
pub fn reset_lock_contention_probe() {
    LOCK_PROBE_HELD.store(0, Ordering::Release);
    LOCK_PROBE_WAITER_CPU.store(0, Ordering::Release);
    LOCK_PROBE_DONE.store(0, Ordering::Release);
    LOCK_PROBE_ERROR.store(0, Ordering::Release);
}

#[cfg(feature = "neo-test-swapgs-irq-ring3")]
pub fn lock_contention_probe_holder(target_cpu: u32) {
    let _guard = SHOOTDOWN_LOCK.lock();
    LOCK_PROBE_HELD.store(1, Ordering::Release);

    for _ in 0..100_000_000 {
        if LOCK_PROBE_WAITER_CPU.load(Ordering::Acquire) == target_cpu + 1 {
            shootdown_while_locked(None, CpuMask::from_cpu(target_cpu));
            LOCK_PROBE_DONE.store(1, Ordering::Release);
            return;
        }
        core::hint::spin_loop();
    }
    LOCK_PROBE_ERROR.store(1, Ordering::Release);
}

#[cfg(feature = "neo-test-swapgs-irq-ring3")]
pub fn lock_contention_probe_held() -> bool {
    LOCK_PROBE_HELD.load(Ordering::Acquire) == 1
}

#[cfg(feature = "neo-test-swapgs-irq-ring3")]
pub fn lock_contention_probe_done() -> bool {
    LOCK_PROBE_DONE.load(Ordering::Acquire) == 1
}

#[cfg(feature = "neo-test-swapgs-irq-ring3")]
pub fn lock_contention_probe_error() -> bool {
    LOCK_PROBE_ERROR.load(Ordering::Acquire) != 0
}

#[cfg(feature = "neo-test-swapgs-irq-ring3")]
pub fn lock_contention_probe_waiter_cpu() -> Option<u32> {
    LOCK_PROBE_WAITER_CPU.load(Ordering::Acquire).checked_sub(1)
}

#[cfg(feature = "neo-test-swapgs-irq-ring3")]
pub fn lock_preserves_interrupt_state_for_test() -> bool {
    let before = arch_x86_64::interrupts_enabled();
    let guard = SHOOTDOWN_LOCK.lock();
    let while_held = arch_x86_64::interrupts_enabled();
    drop(guard);
    before == while_held && before == arch_x86_64::interrupts_enabled()
}

pub fn handle_ipi() {
    let pc = per_cpu::current_per_cpu();
    let payload_ptr = pc.pending_shootdown.swap(ptr::null_mut(), Ordering::AcqRel);
    if payload_ptr.is_null() {
        return;
    }

    #[cfg(feature = "neo-test-instrumentation")]
    pc.shootdown_ipis_received.fetch_add(1, Ordering::Relaxed);
    #[cfg(feature = "neo-test-instrumentation")]
    record_ipi_receipt(pc);

    // SAFETY: senders publish a stack-local payload pointer only while holding
    // SHOOTDOWN_LOCK, then wait for every target to ACK before returning. The
    // receiver swaps its slot to null before using the pointer, so the slot
    // cannot keep a stale reference after the ACK.
    let payload = unsafe { &*payload_ptr };
    debug_assert!(payload.targets.contains(per_cpu::current_cpu_id()));

    invalidate_local(payload.range);
    payload.pending.fetch_sub(1, Ordering::Release);
}

pub fn other_cpus_mask() -> CpuMask {
    shootdown_ready_cpus_mask().without_cpu(per_cpu::current_cpu_id())
}

pub fn shootdown_ready_cpus_mask() -> CpuMask {
    let mut mask = CpuMask::empty();
    for cpu_id in 0..per_cpu::MAX_CPUS as u32 {
        // Scheduler routing uses scheduler_ptr; TLB shootdown targets require
        // the later point where the CPU can actually service shootdown IPIs.
        if per_cpu::tlb_shootdown_ready(cpu_id) {
            mask.insert(cpu_id);
        }
    }
    mask
}

fn publish_payload_to_targets(payload_ptr: *mut ShootdownPayload, targets: CpuMask) {
    for cpu_id in targets.iter() {
        let old = per_cpu::PER_CPU[cpu_id as usize]
            .pending_shootdown
            .swap(payload_ptr, Ordering::AcqRel);
        if !old.is_null() {
            panic!("tlb_shootdown_slot_busy target_cpu={}", cpu_id);
        }
    }
}

fn wait_for_acks_or_panic(payload: &ShootdownPayload, targets: CpuMask) {
    let payload_ptr = payload as *const ShootdownPayload as *mut ShootdownPayload;
    let start = read_tsc();
    let retry_after_cycles = timeout_cycles_for_ms(SHOOTDOWN_RETRY_AFTER_MS);
    let final_timeout_cycles = timeout_cycles_for_ms(SHOOTDOWN_FINAL_TIMEOUT_MS);
    let mut repoked = CpuMask::empty();
    loop {
        let pending = payload.pending.load(Ordering::Acquire);
        if pending == 0 {
            return;
        }
        let elapsed = read_tsc().wrapping_sub(start);
        if elapsed >= final_timeout_cycles {
            #[cfg(feature = "neo-test-instrumentation")]
            record_timeout(targets);
            let unacked_slots = pending_slot_mask(payload_ptr, targets);
            crate::serial_println!(
                "tlb_shootdown_timeout sender_cpu={} sender_if={} pending={} targets=0x{:016x} unacked_slots=0x{:016x} repoked=0x{:016x} elapsed_cycles={} timeout_cycles={}",
                per_cpu::current_cpu_id(),
                arch_x86_64::interrupts_enabled(),
                pending,
                targets.bits(),
                unacked_slots.bits(),
                repoked.bits(),
                elapsed,
                final_timeout_cycles
            );
            panic!(
                "tlb_shootdown_timeout pending={} targets=0x{:016x} unacked_slots=0x{:016x} repoked=0x{:016x} elapsed_cycles={} timeout_cycles={}",
                pending,
                targets.bits(),
                unacked_slots.bits(),
                repoked.bits(),
                elapsed,
                final_timeout_cycles
            );
        }
        if elapsed >= retry_after_cycles {
            repoke_pending_targets(payload_ptr, targets, &mut repoked);
        }
        core::hint::spin_loop();
    }
}

fn repoke_pending_targets(
    payload_ptr: *mut ShootdownPayload,
    targets: CpuMask,
    repoked: &mut CpuMask,
) {
    for cpu_id in targets.iter() {
        if repoked.contains(cpu_id) {
            continue;
        }
        let pc = &per_cpu::PER_CPU[cpu_id as usize];
        if pc.pending_shootdown.load(Ordering::Acquire) != payload_ptr {
            continue;
        }
        repoked.insert(cpu_id);
        #[cfg(feature = "neo-test-instrumentation")]
        pc.shootdown_retry_ipis_sent.fetch_add(1, Ordering::Relaxed);
        apic::send_tlb_shootdown_ipi(cpu_id);
    }
}

fn pending_slot_mask(payload_ptr: *mut ShootdownPayload, targets: CpuMask) -> CpuMask {
    let mut mask = CpuMask::empty();
    for cpu_id in targets.iter() {
        if per_cpu::PER_CPU[cpu_id as usize]
            .pending_shootdown
            .load(Ordering::Acquire)
            == payload_ptr
        {
            mask.insert(cpu_id);
        }
    }
    mask
}

fn invalidate_local(range: Option<(VirtAddr, VirtAddr)>) {
    match range {
        None => flush_local_full(),
        Some((start, end)) => invalidate_local_range(start, end),
    }
}

fn invalidate_local_range(start: VirtAddr, end: VirtAddr) {
    let Some((mut page, end_page, pages)) = normalized_page_range(start, end) else {
        return;
    };

    if pages > INVLPG_THRESHOLD_PAGES {
        flush_local_full();
        return;
    }

    while page < end_page {
        arch_x86_64::invlpg(VirtAddr::new(page));
        page += PAGE_SIZE;
    }

    #[cfg(feature = "neo-test-instrumentation")]
    per_cpu::current_per_cpu()
        .shootdown_invlpg_pages
        .fetch_add(pages, Ordering::Relaxed);
}

fn normalized_page_range(start: VirtAddr, end: VirtAddr) -> Option<(u64, u64, u64)> {
    let start = start.as_u64();
    let end = end.as_u64();
    if end <= start {
        return None;
    }

    let page_start = start & !(PAGE_SIZE - 1);
    let end_minus_one = end - 1;
    let page_end = (end_minus_one & !(PAGE_SIZE - 1)) + PAGE_SIZE;
    let pages = (page_end - page_start) / PAGE_SIZE;
    Some((page_start, page_end, pages))
}

fn flush_local_full() {
    arch_x86_64::flush_local_tlb();
    #[cfg(feature = "neo-test-instrumentation")]
    per_cpu::current_per_cpu()
        .shootdown_full_flushes
        .fetch_add(1, Ordering::Relaxed);
}

fn timeout_cycles_for_ms(ms: u64) -> u64 {
    tsc_cycles_per_ms()
        .unwrap_or(FALLBACK_TSC_CYCLES_PER_MS)
        .saturating_mul(ms)
}

fn tsc_cycles_per_ms() -> Option<u64> {
    let max_leaf = cpuid(0, 0).0;
    if max_leaf >= 0x15 {
        let (denom, numer, crystal_hz, _) = cpuid(0x15, 0);
        if denom != 0 && numer != 0 && crystal_hz != 0 {
            let hz = (crystal_hz as u64)
                .saturating_mul(numer as u64)
                .checked_div(denom as u64)?;
            return Some(hz / 1000);
        }
    }
    if max_leaf >= 0x16 {
        let (base_mhz, _, _, _) = cpuid(0x16, 0);
        if base_mhz != 0 {
            return Some(base_mhz as u64 * 1000);
        }
    }
    None
}

fn cpuid(leaf: u32, subleaf: u32) -> (u32, u32, u32, u32) {
    let eax: u32;
    let ebx: u32;
    let ecx: u32;
    let edx: u32;
    unsafe {
        asm!(
            "push rbx",
            "cpuid",
            "mov {ebx_out:e}, ebx",
            "pop rbx",
            inout("eax") leaf => eax,
            inout("ecx") subleaf => ecx,
            lateout("edx") edx,
            ebx_out = lateout(reg) ebx,
            options(nomem)
        );
    }
    (eax, ebx, ecx, edx)
}

fn read_tsc() -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe {
        asm!(
            "rdtsc",
            out("eax") lo,
            out("edx") hi,
            options(nomem, nostack, preserves_flags)
        );
    }
    ((hi as u64) << 32) | lo as u64
}

#[cfg(feature = "neo-test-instrumentation")]
#[derive(Copy, Clone)]
pub struct ShootdownTestCounters {
    pub ipis_received: u64,
    pub last_ack_cycles: u64,
    pub max_ack_cycles: u64,
    pub retry_ipis_sent: u64,
    pub timeouts: u64,
    pub self_invalidations: u64,
    pub full_flushes: u64,
    pub invlpg_pages: u64,
}

#[cfg(feature = "neo-test-instrumentation")]
pub fn reset_test_counters() {
    for pc in per_cpu::PER_CPU.iter() {
        pc.shootdown_ipis_received.store(0, Ordering::Relaxed);
        pc.shootdown_request_sent_tsc.store(0, Ordering::Relaxed);
        pc.shootdown_last_ack_cycles.store(0, Ordering::Relaxed);
        pc.shootdown_max_ack_cycles.store(0, Ordering::Relaxed);
        pc.shootdown_retry_ipis_sent.store(0, Ordering::Relaxed);
        pc.shootdown_timeouts.store(0, Ordering::Relaxed);
        pc.shootdown_self_invalidations.store(0, Ordering::Relaxed);
        pc.shootdown_full_flushes.store(0, Ordering::Relaxed);
        pc.shootdown_invlpg_pages.store(0, Ordering::Relaxed);
    }
}

#[cfg(feature = "neo-test-instrumentation")]
pub fn test_counters(cpu_id: u32) -> Option<ShootdownTestCounters> {
    if cpu_id as usize >= per_cpu::MAX_CPUS {
        return None;
    }
    let pc = &per_cpu::PER_CPU[cpu_id as usize];
    Some(ShootdownTestCounters {
        ipis_received: pc.shootdown_ipis_received.load(Ordering::Relaxed),
        last_ack_cycles: pc.shootdown_last_ack_cycles.load(Ordering::Relaxed),
        max_ack_cycles: pc.shootdown_max_ack_cycles.load(Ordering::Relaxed),
        retry_ipis_sent: pc.shootdown_retry_ipis_sent.load(Ordering::Relaxed),
        timeouts: pc.shootdown_timeouts.load(Ordering::Relaxed),
        self_invalidations: pc.shootdown_self_invalidations.load(Ordering::Relaxed),
        full_flushes: pc.shootdown_full_flushes.load(Ordering::Relaxed),
        invlpg_pages: pc.shootdown_invlpg_pages.load(Ordering::Relaxed),
    })
}

#[cfg(feature = "neo-test-instrumentation")]
fn record_self_invalidation() {
    per_cpu::current_per_cpu()
        .shootdown_self_invalidations
        .fetch_add(1, Ordering::Relaxed);
}

#[cfg(feature = "neo-test-instrumentation")]
pub fn test_final_timeout_cycles() -> u64 {
    timeout_cycles_for_ms(SHOOTDOWN_FINAL_TIMEOUT_MS)
}

#[cfg(feature = "neo-test-instrumentation")]
fn record_initial_ipi_send(cpu_id: u32) {
    per_cpu::PER_CPU[cpu_id as usize]
        .shootdown_request_sent_tsc
        .store(read_tsc(), Ordering::Relaxed);
}

#[cfg(feature = "neo-test-instrumentation")]
fn record_ipi_receipt(pc: &per_cpu::PerCpu) {
    let sent = pc.shootdown_request_sent_tsc.load(Ordering::Relaxed);
    if sent == 0 {
        return;
    }
    let delta = read_tsc().wrapping_sub(sent);
    pc.shootdown_last_ack_cycles.store(delta, Ordering::Relaxed);
    let mut observed = pc.shootdown_max_ack_cycles.load(Ordering::Relaxed);
    while delta > observed {
        match pc.shootdown_max_ack_cycles.compare_exchange_weak(
            observed,
            delta,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => break,
            Err(next) => observed = next,
        }
    }
}

#[cfg(feature = "neo-test-instrumentation")]
fn record_timeout(targets: CpuMask) {
    for cpu_id in targets.iter() {
        per_cpu::PER_CPU[cpu_id as usize]
            .shootdown_timeouts
            .fetch_add(1, Ordering::Relaxed);
    }
}
