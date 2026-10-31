// SPDX-License-Identifier: MPL-2.0
//! Arthur answer-latency breakdown (feature `prefill-perf`; in image A's
//! kernel feature list).
//!
//! Where one `sys_generate` spends its time, split so a photo of the screen
//! answers the diagnosis questions in
//! `Plans/NOTE-2026-10-23-arthur-prefill-latency.md` (round 2: on silicon):
//!
//! - prefill vs decode: tokens and cycles of each;
//! - per forward: the 24-layer stack vs the `lm_head` (vocab) projection;
//! - quantized matvec cycles, the SwiGLU element-wise step and the attention
//!   core, so the rest of a layer (norms, RoPE, residuals) is the difference;
//! - the `par_matmul` fork-join: jobs, cycles the caller spends in the barrier
//!   after its own partition, partitions it stole back, partitions APs ran;
//! - the timeline in the overlay's seconds: Arthur's last surface commit
//!   before the generation ("Thinking..."), the generation, the first frame
//!   the WM presented after it started, the longest gap between presents
//!   while it ran, and the first present after it ended (the answer);
//! - pointer input while it ran: the records the kernel minted for the WM's
//!   pointer route (`ptr in`, one per report the touchpad or USB mouse
//!   delivered; `atrium-pointer-forward`, the product's), and how many of
//!   those the WM dequeued (`rx`). With `presents`, they split "nothing
//!   moved" (`in=0`) from "the WM did not run" (`in>0 rx=0`) from "the WM ran
//!   but drew nothing". `rx` never exceeds `in`: one published window, both
//!   counted at the pointer route (`pointer_window_logic`; Janus, #91);
//! - per CPU: partitions run, cycles per partition, and the effective clock
//!   from APERF/MPERF while running them; the generating CPU's clock over the
//!   whole generation; and its HWP state (`arch::x86_64::cpufreq`).
//!
//! Every probe is a relaxed atomic add or store of a TSC or MSR reading.
//! Nothing here takes a lock, parks, or changes what the forward computes, so
//! the logits are bit-identical with the feature on or off (the
//! `forward-perf` precedent). The overlay reads it from the watchdog AP.
//! Feature off: none of this is compiled and no call site exists.

use core::sync::atomic::{AtomicU8, AtomicU32, AtomicU64, Ordering};

use crate::arch::x86_64::cpufreq;
use crate::arch::x86_64::per_cpu::MAX_CPUS;

#[inline]
pub fn tsc() -> u64 {
    crate::ai::forward_perf::read_tsc()
}

macro_rules! counter {
    ($($name:ident),* $(,)?) => { $(static $name: AtomicU64 = AtomicU64::new(0);)* };
}

/// The last surface commit per process (slot = pid % [`COMMIT_SLOTS`]): the
/// WM commits its own output buffer right after presenting Arthur's, so a
/// single "last commit" would always be the WM's and `think` never Arthur's.
///
/// Each slot is ONE word, `tsc & !PID_MASK | pid & PID_MASK`, written by one
/// store, so two processes committing into the same slot on different CPUs
/// can only evict each other (the later wins); a generation whose pid is not
/// in its slot reads no `think`, never another process's time. The TSC keeps
/// its high 44 bits (2^20 cycles, 1.05 ms at 1 GHz). Two pids that share
/// a slot and their low 20 bits differ by a multiple of 2^20; pids are
/// allocated monotonically, so that takes over a million processes.
const COMMIT_SLOTS: usize = 8;
static COMMIT: [AtomicU64; COMMIT_SLOTS] = [const { AtomicU64::new(0) }; COMMIT_SLOTS];

counter!(
    PREFILL_TOKENS,
    PREFILL_CYCLES,
    DECODE_TOKENS,
    DECODE_CYCLES,
    LAYERS_CYCLES,
    LM_HEAD_CYCLES,
    MATVEC_CYCLES,
    MATVEC_CALLS,
    SWIGLU_CYCLES,
    ATTN_CYCLES,
    PAR_JOBS,
    PAR_BARRIER_CYCLES,
    // The timeline (TSC values; 0 = not seen).
    GEN_BEGIN_TSC,
    PREFILL_END_TSC,
    GEN_END_TSC,
    THINK_TSC,
    FIRST_PRESENT_TSC,
    LAST_PRESENT_TSC,
    MAX_PRESENT_GAP,
    PRESENTS,
    ANSWER_PRESENT_TSC,
    GEN_PID,
    // Pointer input while it ran: driver reports at the start, the count
    // over the generation, and the records the WM received.
    // The generating CPU over the whole generation.
    GEN_APERF0,
    GEN_MPERF0,
    GEN_D_APERF,
    GEN_D_MPERF,
    HWP_REQUEST,
    HWP_CAPS,
);

static GEN_CPU: AtomicU32 = AtomicU32::new(u32::MAX);
/// bit 0: HWP supported, bit 1: enabled; bits 15:8: IA32_PERF_STATUS ratio.
static HWP_STATE: AtomicU32 = AtomicU32::new(0);

/// Per CPU: partitions run, their TSC cycles, and APERF/MPERF while running.
static PARTS: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(0) }; MAX_CPUS];
static PART_CYCLES: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(0) }; MAX_CPUS];
static PART_APERF: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(0) }; MAX_CPUS];
static PART_MPERF: [AtomicU64; MAX_CPUS] = [const { AtomicU64::new(0) }; MAX_CPUS];

/// Per CPU, whether its own CPUID reports APERF/MPERF: 0 not asked yet, 1 no,
/// 2 yes. Each CPU asks once, for itself (a capability of CPU-local state is
/// decided per CPU, CP-79).
static APERF_OK: [AtomicU8; MAX_CPUS] = [const { AtomicU8::new(0) }; MAX_CPUS];

#[inline]
fn add(counter: &AtomicU64, v: u64) {
    counter.fetch_add(v, Ordering::Relaxed);
}

fn cpu() -> usize {
    (crate::arch::x86_64::per_cpu::current_cpu_id() as usize).min(MAX_CPUS - 1)
}

/// This CPU's (APERF, MPERF), when its CPUID reports them.
fn aperf_mperf(cpu: usize) -> Option<(u64, u64)> {
    let state = match APERF_OK[cpu].load(Ordering::Relaxed) {
        0 => {
            let ok = if cpufreq::caps().aperf_mperf { 2 } else { 1 };
            APERF_OK[cpu].store(ok, Ordering::Relaxed);
            ok
        }
        s => s,
    };
    if state != 2 {
        return None;
    }
    cpufreq::aperf_mperf(cpufreq::PowerCaps {
        aperf_mperf: true,
        ..cpufreq::PowerCaps::default()
    })
}

/// A new generation: clear the per-generation counters and stamp its start.
pub fn begin_generation() {
    for c in [
        &PREFILL_TOKENS,
        &PREFILL_CYCLES,
        &DECODE_TOKENS,
        &DECODE_CYCLES,
        &LAYERS_CYCLES,
        &LM_HEAD_CYCLES,
        &MATVEC_CYCLES,
        &MATVEC_CALLS,
        &SWIGLU_CYCLES,
        &ATTN_CYCLES,
        &PAR_JOBS,
        &PAR_BARRIER_CYCLES,
        &PREFILL_END_TSC,
        &GEN_END_TSC,
        &THINK_TSC,
        &FIRST_PRESENT_TSC,
        &LAST_PRESENT_TSC,
        &MAX_PRESENT_GAP,
        &PRESENTS,
        &ANSWER_PRESENT_TSC,
        &GEN_D_APERF,
        &GEN_D_MPERF,
    ] {
        c.store(0, Ordering::Relaxed);
    }
    for i in 0..MAX_CPUS {
        PARTS[i].store(0, Ordering::Relaxed);
        PART_CYCLES[i].store(0, Ordering::Relaxed);
        PART_APERF[i].store(0, Ordering::Relaxed);
        PART_MPERF[i].store(0, Ordering::Relaxed);
    }
    crate::ai::par_matmul::reset_counters();

    let cpu = cpu();
    GEN_CPU.store(cpu as u32, Ordering::Relaxed);
    let caps = cpufreq::caps();
    let hwp = cpufreq::hwp_snapshot(caps);
    HWP_STATE.store(
        (hwp.supported as u32) | ((hwp.enabled as u32) << 1) | ((hwp.ratio as u32) << 8),
        Ordering::Relaxed,
    );
    HWP_REQUEST.store(hwp.request, Ordering::Relaxed);
    HWP_CAPS.store(hwp.caps, Ordering::Relaxed);
    let (a, m) = aperf_mperf(cpu).unwrap_or((0, 0));
    GEN_APERF0.store(a, Ordering::Relaxed);
    GEN_MPERF0.store(m, Ordering::Relaxed);

    let pid = crate::sched::current_pid().0 as u64;
    GEN_PID.store(pid, Ordering::Relaxed);
    let slot = (pid as usize) % COMMIT_SLOTS;
    if let Some(at) =
        super::commit_slot_logic::commit_tsc(COMMIT[slot].load(Ordering::Relaxed), pid)
    {
        THINK_TSC.store(at, Ordering::Relaxed);
    }
    GEN_BEGIN_TSC.store(tsc(), Ordering::Relaxed);
    PTR.open();
}

pub fn prefill_done(tokens: usize, cycles: u64) {
    add(&PREFILL_TOKENS, tokens as u64);
    add(&PREFILL_CYCLES, cycles);
    PREFILL_END_TSC.store(tsc(), Ordering::Relaxed);
}

/// The generation ended (answer or error). The generating CPU's clock is
/// taken only if the call is still on the CPU it started on.
pub fn end_generation() {
    let cpu = cpu();
    if GEN_CPU.load(Ordering::Relaxed) == cpu as u32
        && let Some((a, m)) = aperf_mperf(cpu)
    {
        GEN_D_APERF.store(
            a.wrapping_sub(GEN_APERF0.load(Ordering::Relaxed)),
            Ordering::Relaxed,
        );
        GEN_D_MPERF.store(
            m.wrapping_sub(GEN_MPERF0.load(Ordering::Relaxed)),
            Ordering::Relaxed,
        );
    }
    PTR.close();
    GEN_END_TSC.store(tsc(), Ordering::Relaxed);
}

/// The generation's pointer window (`ptr in` / `rx`).
static PTR: super::pointer_window_logic::PointerWindow =
    super::pointer_window_logic::PointerWindow::new();

/// The kernel minted pointer record `tag` for the WM (under `DELIVERY`).
pub fn note_pointer_minted(tag: u16) {
    PTR.note_minted(tag);
}

/// The WM dequeued pointer record `tag` (under `DELIVERY`).
pub fn note_pointer_rx(tag: u16) {
    PTR.note_rx(tag);
}

pub fn decode_token(cycles: u64) {
    add(&DECODE_TOKENS, 1);
    add(&DECODE_CYCLES, cycles);
}

pub fn layers(cycles: u64) {
    add(&LAYERS_CYCLES, cycles);
}

pub fn lm_head(cycles: u64) {
    add(&LM_HEAD_CYCLES, cycles);
}

pub fn matvec(cycles: u64) {
    add(&MATVEC_CYCLES, cycles);
    add(&MATVEC_CALLS, 1);
}

pub fn swiglu(cycles: u64) {
    add(&SWIGLU_CYCLES, cycles);
}

pub fn attn(cycles: u64) {
    add(&ATTN_CYCLES, cycles);
}

pub fn par_job(barrier_cycles: u64) {
    add(&PAR_JOBS, 1);
    add(&PAR_BARRIER_CYCLES, barrier_cycles);
}

/// Taken before a fork-join partition runs, on the CPU that runs it.
pub struct PartStamp {
    cpu: usize,
    tsc: u64,
    am: Option<(u64, u64)>,
}

#[inline]
pub fn partition_begin() -> PartStamp {
    let cpu = cpu();
    PartStamp {
        cpu,
        am: aperf_mperf(cpu),
        tsc: tsc(),
    }
}

#[inline]
pub fn partition_end(stamp: PartStamp) {
    let c = stamp.cpu;
    add(&PART_CYCLES[c], tsc().wrapping_sub(stamp.tsc));
    add(&PARTS[c], 1);
    if let (Some((a0, m0)), Some((a1, m1))) = (stamp.am, aperf_mperf(c)) {
        add(&PART_APERF[c], a1.wrapping_sub(a0));
        add(&PART_MPERF[c], m1.wrapping_sub(m0));
    }
}

/// Without the compositor syscalls there is nothing to note.
#[cfg(not(feature = "atrium-compositor-g1"))]
pub fn note_syscall(_pid: u64, _nr: u64) {}

/// Every syscall entry, from `watchdog::note_syscall` (with
/// `freeze-watchdog`): the surface commits and the WM's presents that make
/// the timeline.
#[cfg(feature = "atrium-compositor-g1")]
pub fn note_syscall(pid: u64, nr: u64) {
    use crate::arch::x86_64::syscall::{
        SYS_BUFFER_COMMIT, SYS_BUFFER_COMMIT_DAMAGE, SYS_FB_PRESENT,
    };
    if nr == SYS_BUFFER_COMMIT || nr == SYS_BUFFER_COMMIT_DAMAGE {
        let slot = (pid as usize) % COMMIT_SLOTS;
        COMMIT[slot].store(
            super::commit_slot_logic::commit_word(pid, tsc()),
            Ordering::Relaxed,
        );
        return;
    }
    if nr != SYS_FB_PRESENT {
        return;
    }
    let begin = GEN_BEGIN_TSC.load(Ordering::Relaxed);
    let now = tsc();
    if begin == 0 || now < begin {
        return;
    }
    if GEN_END_TSC.load(Ordering::Relaxed) == 0 {
        add(&PRESENTS, 1);
        let _ = FIRST_PRESENT_TSC.compare_exchange(0, now, Ordering::Relaxed, Ordering::Relaxed);
        let last = LAST_PRESENT_TSC.swap(now, Ordering::Relaxed).max(begin);
        MAX_PRESENT_GAP.fetch_max(now.saturating_sub(last), Ordering::Relaxed);
    } else {
        let _ = ANSWER_PRESENT_TSC.compare_exchange(0, now, Ordering::Relaxed, Ordering::Relaxed);
    }
}

fn hz() -> u64 {
    let hz = crate::ai::forward_perf::tsc_hz();
    if hz != 0 { hz } else { 2_000_000_000 }
}

fn ms(cycles: u64) -> u64 {
    cycles.saturating_mul(1000) / hz()
}

fn us(cycles: u64) -> u64 {
    cycles.saturating_mul(1_000_000) / hz()
}

/// `t` relative to `base`, in ms, or `-` when `t` was not seen.
struct Rel(u64, u64);

impl core::fmt::Display for Rel {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.0 == 0 || self.1 == 0 {
            f.write_str("-")
        } else if self.0 >= self.1 {
            write!(f, "+{}ms", ms(self.0 - self.1))
        } else {
            write!(f, "-{}ms", ms(self.1 - self.0))
        }
    }
}

/// One line: `perf pre n=28 ms/t=… | dec n=7 ms/t=… | lm=…% mv=…% sw=…% at=…% | par j=… bar=…% st=… ap=…`
/// (`prefill_perf_logic::write_perf_line`).
pub fn write_line(out: &mut dyn core::fmt::Write) -> core::fmt::Result {
    let line = crate::ai::prefill_perf_logic::PerfLine {
        tsc_hz: hz(),
        prefill_tokens: PREFILL_TOKENS.load(Ordering::Relaxed),
        prefill_cycles: PREFILL_CYCLES.load(Ordering::Relaxed),
        decode_tokens: DECODE_TOKENS.load(Ordering::Relaxed),
        decode_cycles: DECODE_CYCLES.load(Ordering::Relaxed),
        layers_cycles: LAYERS_CYCLES.load(Ordering::Relaxed),
        lm_head_cycles: LM_HEAD_CYCLES.load(Ordering::Relaxed),
        matvec_cycles: MATVEC_CYCLES.load(Ordering::Relaxed),
        swiglu_cycles: SWIGLU_CYCLES.load(Ordering::Relaxed),
        attn_cycles: ATTN_CYCLES.load(Ordering::Relaxed),
        par_jobs: PAR_JOBS.load(Ordering::Relaxed),
        par_barrier_cycles: PAR_BARRIER_CYCLES.load(Ordering::Relaxed),
        steals: crate::ai::par_matmul::bsp_steals() as u64,
        ap_parts: crate::ai::par_matmul::ap_partitions_done() as u64,
    };
    crate::ai::prefill_perf_logic::write_perf_line(out, &line)
}

/// The timeline line, in the overlay's seconds (`start_tsc` is its `t=0`):
/// `time gen@t=…s think=-…ms | pre=…ms dec=…ms | shown=+…ms presents=… gap=…ms | ptr in=… rx=… | answer=+…ms`.
/// `shown` is the WM's first present after the generation started, `gap` the
/// longest stretch without one while it ran (the whole generation if none),
/// `answer` the first present after it ended, relative to its end.
pub fn write_timeline(out: &mut dyn core::fmt::Write, start_tsc: u64) -> core::fmt::Result {
    let begin = GEN_BEGIN_TSC.load(Ordering::Relaxed);
    if begin == 0 {
        return out.write_str("time: no generation yet");
    }
    let pre_end = PREFILL_END_TSC.load(Ordering::Relaxed);
    let end = GEN_END_TSC.load(Ordering::Relaxed);
    let presents = PRESENTS.load(Ordering::Relaxed);
    let gap = if presents == 0 {
        let stop = if end != 0 { end } else { tsc() };
        stop.saturating_sub(begin)
    } else {
        MAX_PRESENT_GAP.load(Ordering::Relaxed)
    };
    write!(
        out,
        "time gen@t={}s think={} | pre={}ms dec={}ms | shown={} presents={} gap={}ms | ptr in={} rx={} | answer={}",
        begin.saturating_sub(start_tsc) / hz(),
        Rel(THINK_TSC.load(Ordering::Relaxed), begin),
        if pre_end != 0 { ms(pre_end - begin) } else { 0 },
        if end != 0 && pre_end != 0 {
            ms(end.saturating_sub(pre_end))
        } else {
            0
        },
        Rel(FIRST_PRESENT_TSC.load(Ordering::Relaxed), begin),
        presents,
        ms(gap),
        PTR.counts().0,
        PTR.counts().1,
        Rel(ANSWER_PRESENT_TSC.load(Ordering::Relaxed), end),
    )
}

/// The CPU line: `cpu gen=cN …MHz | parts own=… ap=… on K cpus | ap MHz min/avg/max | part us min..max (cN) | hwp …`.
pub fn write_cpus(out: &mut dyn core::fmt::Write, online: usize) -> core::fmt::Result {
    let gen_cpu = GEN_CPU.load(Ordering::Relaxed) as usize;
    let hz = hz();
    let gen_mhz = cpufreq::effective_mhz(
        hz,
        GEN_D_APERF.load(Ordering::Relaxed),
        GEN_D_MPERF.load(Ordering::Relaxed),
    );
    let (mut own, mut ap, mut cpus) = (0u64, 0u64, 0u32);
    let (mut mhz_min, mut mhz_max, mut mhz_sum, mut mhz_n) = (u64::MAX, 0u64, 0u64, 0u64);
    let (mut part_min, mut part_max, mut slow) = (u64::MAX, 0u64, 0usize);
    for c in 0..online.min(MAX_CPUS) {
        let n = PARTS[c].load(Ordering::Relaxed);
        if n == 0 {
            continue;
        }
        cpus += 1;
        if c == gen_cpu {
            own += n;
        } else {
            ap += n;
            if let Some(mhz) = cpufreq::effective_mhz(
                hz,
                PART_APERF[c].load(Ordering::Relaxed),
                PART_MPERF[c].load(Ordering::Relaxed),
            ) {
                mhz_min = mhz_min.min(mhz);
                mhz_max = mhz_max.max(mhz);
                mhz_sum += mhz;
                mhz_n += 1;
            }
        }
        let per = us(PART_CYCLES[c].load(Ordering::Relaxed)) / n;
        part_min = part_min.min(per);
        if per > part_max {
            part_max = per;
            slow = c;
        }
    }
    write!(out, "cpu gen=c{} ", gen_cpu)?;
    match gen_mhz {
        Some(m) => write!(out, "{}MHz", m)?,
        None => out.write_str("MHz=n/a")?,
    }
    write!(out, " | parts own={} ap={} on {} cpus", own, ap, cpus)?;
    if let Some(avg) = mhz_sum.checked_div(mhz_n) {
        write!(out, " | ap MHz {}/{}/{}", mhz_min, avg, mhz_max)?;
    }
    if cpus != 0 {
        write!(out, " | part us {}..{} (c{})", part_min, part_max, slow)?;
    }
    let hwp = HWP_STATE.load(Ordering::Relaxed);
    if hwp & 1 != 0 {
        write!(
            out,
            " | hwp={} req={:#x} caps={:#x}",
            if hwp & 2 != 0 { "on" } else { "off" },
            HWP_REQUEST.load(Ordering::Relaxed),
            HWP_CAPS.load(Ordering::Relaxed) & 0xffff_ffff
        )?;
    } else {
        out.write_str(" | hwp=none")?;
    }
    write!(out, " ratio={}", (hwp >> 8) & 0xff)?;
    #[cfg(feature = "hwp-performance")]
    {
        let (was, turned) = cpufreq::hwp_counts();
        write!(out, " perf-req was={} set={}", was, turned)?;
    }
    Ok(())
}
