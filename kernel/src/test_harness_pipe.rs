// SPDX-License-Identifier: MPL-2.0
// L1a.1 step 2a: the pipe scenarios, `pipe_core` and `pipe_park_kill`
// (docs/THINK-2026-11-02-l1a1-pipe-wiring.md §3, §5).
//
// `include!`d into `test_harness.rs` (as `test_harness_latency_oracle.rs`),
// so these items share the harness's `fail`, `pass` and `serial_println!`.
// Comments here are `//`, not `//!`: inner doc comments are invalid mid-module.
//
// The callers are stand-in processes (as `ipc_park_kill`'s): each has its own
// pid and address space with one user page at `ADMISSION_KILL_VA`, and one
// kernel-built thread that makes one syscall through the real dispatcher, so
// the pipe syscalls run their user copies, handle lookups, anchors and parks
// exactly as from ring 3. The harness reaches each page through the HHDM.
// Where a leg needs an endpoint held without a thread, the harness holds it
// itself (`PipeObject::retain` / `release`, what a handle install does).

// Page layout: wait items at +0, the I/O buffer at +0x100.
#[cfg(any(feature = "neo-test-pipe-core", feature = "neo-test-pipe-park-kill"))]
const PT_BUF: u64 = 0x100;

/// (pid, syscall, args, result, done) per stand-in caller.
#[cfg(any(feature = "neo-test-pipe-core", feature = "neo-test-pipe-park-kill"))]
#[allow(clippy::type_complexity)]
static PT_CALLS: crate::sync::SwitchMutex<[(u32, u64, [u64; 4], i64, bool); 24]> =
    crate::sync::SwitchMutex::new([(0, 0, [0; 4], 0, false); 24]);

#[cfg(any(feature = "neo-test-pipe-core", feature = "neo-test-pipe-park-kill"))]
extern "C" fn pt_caller() -> ! {
    use crate::arch::x86_64::syscall::syscall_dispatch4_for_test;
    let pid = crate::sched::current_pid().0;
    let call = crate::arch::x86_64::without_interrupts(|| {
        PT_CALLS.lock().iter().find(|c| c.0 == pid).copied()
    })
    .unwrap_or_else(|| fail("reason=pipe_caller_unknown"));
    let a = call.2;
    // A real `syscall` enters the dispatcher with IF=0 (SFMASK masks IF),
    // and the token park requires it.
    let ret = crate::arch::x86_64::without_interrupts(|| {
        syscall_dispatch4_for_test(call.1, a[0], a[1], a[2], a[3])
    });
    crate::arch::x86_64::without_interrupts(|| {
        if let Some(c) = PT_CALLS.lock().iter_mut().find(|c| c.0 == pid) {
            c.3 = ret;
            c.4 = true;
        }
    });
    crate::sched::exit();
}

/// A stand-in process with its user page (filled with 0x5a); returns it and
/// the page's HHDM alias.
#[cfg(any(feature = "neo-test-pipe-core", feature = "neo-test-pipe-park-kill"))]
fn pt_process() -> (alloc::sync::Arc<crate::sched::process::Process>, *mut u8) {
    use crate::arch::x86_64::syscall::admission_kill_process_for_test;
    let page = crate::mm::with_frame_alloc(|fa| {
        use x86_64::structures::paging::FrameAllocator;
        fa.allocate_frame()
    })
        .unwrap_or_else(|| fail("reason=pipe_out_of_frames"));
    let proc = admission_kill_process_for_test(page);
    let base = crate::mm::phys_to_virt(page.start_address()).as_mut_ptr::<u8>();
    // SAFETY: a fresh frame, mapped as the stand-in's user page, written
    // through its HHDM alias before any thread of it exists.
    unsafe { core::ptr::write_bytes(base, 0x5a, 4096) };
    (proc, base)
}

/// Install a handle on `end` of `pipe` in `proc`; returns its index.
#[cfg(any(feature = "neo-test-pipe-core", feature = "neo-test-pipe-park-kill"))]
fn pt_install(
    proc: &alloc::sync::Arc<crate::sched::process::Process>,
    pipe: &alloc::sync::Arc<crate::ipc::pipe::PipeObject>,
    end: crate::ipc::pipe_logic::End,
) -> u32 {
    use crate::sched::process::{self, Handle};
    process::install_cap(
        proc,
        Handle::Pipe {
            pipe: pipe.clone(),
            end,
            cap_id: process::alloc_cap_id(),
            rights: process::full_pipe_rights(end),
        },
        None,
    )
    .unwrap_or_else(|_| fail("reason=pipe_install_failed"))
}

/// Start `proc`'s one thread on `cpu`, making syscall `num(args)`. Returns
/// its pid (the harness keeps no `Arc<Process>`).
#[cfg(any(feature = "neo-test-pipe-core", feature = "neo-test-pipe-park-kill"))]
fn pt_start(
    proc: alloc::sync::Arc<crate::sched::process::Process>,
    num: u64,
    args: [u64; 4],
    cpu: u32,
) -> u32 {
    let pid = proc.pid;
    let pmap = proc
        .pmap
        .clone()
        .unwrap_or_else(|| fail("reason=pipe_no_pmap"));
    drop(proc);
    crate::arch::x86_64::without_interrupts(|| {
        let mut calls = PT_CALLS.lock();
        let slot = calls
            .iter_mut()
            .find(|c| c.0 == 0)
            .unwrap_or_else(|| fail("reason=pipe_caller_table_full"));
        *slot = (pid.0, num, args, 0, false);
    });
    let mut thread = crate::sched::thread::Thread::new("pipe-caller", pt_caller);
    thread.bind_process_for_test(pid, pmap);
    if crate::sched::scheduler::place_thread_on_cpu(thread, cpu).is_err() {
        fail("reason=pipe_place_thread_failed");
    }
    pid.0
}

/// A reader of `n` bytes into its page's buffer (`flags` 0: blocking).
#[cfg(any(feature = "neo-test-pipe-core", feature = "neo-test-pipe-park-kill"))]
fn pt_reader(
    pipe: &alloc::sync::Arc<crate::ipc::pipe::PipeObject>,
    n: u64,
    flags: u64,
) -> (u32, *mut u8) {
    use crate::arch::x86_64::syscall::{ADMISSION_KILL_VA, SYS_PIPE_READ};
    let (proc, base) = pt_process();
    let h = pt_install(&proc, pipe, crate::ipc::pipe_logic::End::Read);
    let pid = pt_start(
        proc,
        SYS_PIPE_READ,
        [h as u64, ADMISSION_KILL_VA + PT_BUF, n, flags],
        0,
    );
    (pid, base)
}

/// A writer of `n` bytes of `byte` from its page's buffer.
#[cfg(any(feature = "neo-test-pipe-core", feature = "neo-test-pipe-park-kill"))]
fn pt_writer(pipe: &alloc::sync::Arc<crate::ipc::pipe::PipeObject>, n: u64, byte: u8) -> u32 {
    use crate::arch::x86_64::syscall::{ADMISSION_KILL_VA, SYS_PIPE_WRITE};
    let (proc, base) = pt_process();
    // SAFETY: the stand-in's own page (4096 bytes), before its thread runs.
    unsafe { core::ptr::write_bytes(base.add(PT_BUF as usize), byte, n as usize) };
    let h = pt_install(&proc, pipe, crate::ipc::pipe_logic::End::Write);
    pt_start(
        proc,
        SYS_PIPE_WRITE,
        [h as u64, ADMISSION_KILL_VA + PT_BUF, n, 0],
        0,
    )
}

#[cfg(any(feature = "neo-test-pipe-core", feature = "neo-test-pipe-park-kill"))]
fn pt_result(pid: u32) -> Option<i64> {
    crate::arch::x86_64::without_interrupts(|| {
        PT_CALLS
            .lock()
            .iter()
            .find(|c| c.0 == pid)
            .and_then(|c| c.4.then_some(c.3))
    })
}

#[cfg(any(feature = "neo-test-pipe-core", feature = "neo-test-pipe-park-kill"))]
fn pt_gone(pid: u32) -> bool {
    crate::sched::process::get(crate::sched::process::ProcessId(pid)).is_none()
}

/// Forget every caller (between legs; their processes are gone).
#[cfg(any(feature = "neo-test-pipe-core", feature = "neo-test-pipe-park-kill"))]
fn pt_reset() {
    crate::arch::x86_64::without_interrupts(|| {
        *PT_CALLS.lock() = [(0, 0, [0; 4], 0, false); 24];
    });
}

/// Run the scheduler until `done`, or fail with `reason` after 300 ticks.
#[cfg(any(feature = "neo-test-pipe-core", feature = "neo-test-pipe-park-kill"))]
fn pt_until(reason: &str, mut done: impl FnMut() -> bool) {
    let deadline = crate::arch::x86_64::timer::TICKS.load(Ordering::Acquire) + 300;
    while !done() {
        if crate::arch::x86_64::timer::TICKS.load(Ordering::Acquire) > deadline {
            fail(reason);
        }
        crate::sched::yield_now();
        crate::sched::reap();
    }
}

/// Let the reaper finish what the last exits left.
#[cfg(any(feature = "neo-test-pipe-core", feature = "neo-test-pipe-park-kill"))]
fn pt_settle() {
    for _ in 0..200 {
        crate::sched::yield_now();
        crate::sched::reap();
    }
}

/// Wait for a caller's result, then for its process to go.
#[cfg(any(feature = "neo-test-pipe-core", feature = "neo-test-pipe-park-kill"))]
fn pt_finish(reason: &str, pid: u32) -> i64 {
    pt_until(reason, || pt_result(pid).is_some() && pt_gone(pid));
    pt_result(pid).unwrap_or(i64::MIN)
}

// ── pipe_core ──

/// The create leg: a stand-in that creates its own pipe, writes three bytes
/// and reads them back (`SYS_PIPE_CREATE`'s two installs, then the same
/// process on both ends), and asks for a pipe of capacity 0.
#[cfg(feature = "neo-test-pipe-core")]
static PT_CREATE: crate::sync::SwitchMutex<[i64; 8]> = crate::sync::SwitchMutex::new([0; 8]);
#[cfg(feature = "neo-test-pipe-core")]
static PT_CREATE_DONE: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);

#[cfg(feature = "neo-test-pipe-core")]
extern "C" fn pt_create_caller() -> ! {
    use crate::arch::x86_64::syscall::{
        ADMISSION_KILL_VA, SYS_CLOSE, SYS_PIPE_CREATE, SYS_PIPE_READ, SYS_PIPE_WRITE,
        syscall_dispatch4_for_test as sys,
    };
    let buf = ADMISSION_KILL_VA + PT_BUF;
    // IF=0, as a real `syscall` enters (see `pt_caller`).
    let sys = |n, a0, a1, a2, a3| {
        crate::arch::x86_64::without_interrupts(|| sys(n, a0, a1, a2, a3))
    };
    let bad = sys(SYS_PIPE_CREATE, 0, 0, 0, 0);
    // Below PIPE_ATOMIC: refused, so a small write is always atomic (Chad
    // #56 item 5).
    let small = sys(SYS_PIPE_CREATE, 511, 0, 0, 0);
    let both = sys(SYS_PIPE_CREATE, 512, 0, 0, 0);
    let (rd, wr) = ((both as u64) & 0xffff_ffff, (both as u64) >> 32);
    let w = sys(SYS_PIPE_WRITE, wr, buf, 3, 0);
    let r = sys(SYS_PIPE_READ, rd, buf + 0x80, 16, 0);
    // The read end can't write (rights).
    let wrong = sys(SYS_PIPE_WRITE, rd, buf, 1, 0);
    // SYS_CLOSE closes a pipe end: the last write end gone, a read is EOF.
    let closed = sys(SYS_CLOSE, wr, 0, 0, 0);
    let eof = sys(SYS_PIPE_READ, rd, buf + 0x80, 16, 0);
    crate::arch::x86_64::without_interrupts(|| {
        *PT_CREATE.lock() = [bad, small, both, w, r, wrong, closed, eof];
    });
    PT_CREATE_DONE.store(true, Ordering::Release);
    crate::sched::exit();
}

/// CP-14 leg: the pipe the writer on CPU 1 writes into, and how many of its
/// writes went in.
#[cfg(feature = "neo-test-pipe-core")]
static PT_RACE_PIPE: crate::sync::SwitchMutex<
    Option<alloc::sync::Arc<crate::ipc::pipe::PipeObject>>,
> = crate::sync::SwitchMutex::new(None);
#[cfg(feature = "neo-test-pipe-core")]
static PT_RACE_WROTE: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "neo-test-pipe-core")]
static PT_RACE_DONE: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "neo-test-pipe-core")]
extern "C" fn pt_race_writer() -> ! {
    use crate::ipc::pipe::race;
    let pipe = PT_RACE_PIPE.lock().clone().expect("race pipe armed");
    // Bounded: if the reader never enters the window the harness fails the
    // round on its own deadline.
    for _ in 0..200_000_000u64 {
        if race::IN_WINDOW.load(Ordering::Acquire) {
            break;
        }
        core::hint::spin_loop();
    }
    race::WRITER_STARTED.store(true, Ordering::Release);
    if pipe.try_write(b"z") == Ok(1) {
        PT_RACE_WROTE.fetch_add(1, Ordering::AcqRel);
    }
    drop(pipe);
    PT_RACE_DONE.fetch_add(1, Ordering::AcqRel);
    crate::sched::exit();
}

/// CP-61 leg: the waiter's handle index, the pipe its slot is reused for,
/// and whether the reinstall landed in the same slot.
#[cfg(feature = "neo-test-pipe-core")]
static PT_REBIND_HANDLE: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
#[cfg(feature = "neo-test-pipe-core")]
static PT_REBIND_PIPE: crate::sync::SwitchMutex<
    Option<alloc::sync::Arc<crate::ipc::pipe::PipeObject>>,
> = crate::sync::SwitchMutex::new(None);
#[cfg(feature = "neo-test-pipe-core")]
static PT_REBIND_SLOT_MOVED: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

/// A second thread of the waiter's process: while the waiter is between its
/// first scan and its registration, close the waiter's read end of pipe A
/// and install a read end of pipe B, which lands in the same slot.
#[cfg(feature = "neo-test-pipe-core")]
extern "C" fn pt_rebind_sibling() -> ! {
    use crate::sched::wait_any::{REBIND_DONE, REBIND_IN_WINDOW};
    while !REBIND_IN_WINDOW.load(Ordering::Acquire) {
        core::hint::spin_loop();
    }
    let proc = crate::sched::process::get(crate::sched::current_pid()).expect("rebind process");
    let h = PT_REBIND_HANDLE.load(Ordering::Acquire);
    let gone = crate::arch::x86_64::without_interrupts(|| proc.handles.lock().remove(h));
    // The pipe A endpoint is released here, outside the handle lock.
    drop(gone);
    let pipe_b = PT_REBIND_PIPE.lock().clone().expect("rebind pipe armed");
    let idx = pt_install(&proc, &pipe_b, crate::ipc::pipe_logic::End::Read);
    PT_REBIND_SLOT_MOVED.store(idx != h, Ordering::Release);
    drop(pipe_b);
    drop(proc);
    REBIND_DONE.store(true, Ordering::Release);
    // Stay alive: the waiter's process must outlive its wait. The harness
    // terminates it.
    loop {
        crate::sched::yield_now();
    }
}

/// A stand-in whose one call is `SYS_WAIT_ANY` on `handle` (a pipe item),
/// with a deadline `ticks` from now. Its page holds the item.
#[cfg(feature = "neo-test-pipe-core")]
fn pt_wait_any(
    proc: alloc::sync::Arc<crate::sched::process::Process>,
    base: *mut u8,
    handle: u32,
    ticks: u64,
) -> u32 {
    use crate::arch::x86_64::syscall::{ADMISSION_KILL_VA, SYS_WAIT_ANY};
    use crate::sched::wait_any_logic::WAIT_KIND_PIPE;
    let item = [handle, WAIT_KIND_PIPE];
    // SAFETY: the stand-in's own page, before its thread runs; 8 bytes at +0.
    unsafe { core::ptr::copy_nonoverlapping(item.as_ptr().cast::<u8>(), base, 8) };
    let now = crate::arch::x86_64::timer::TICKS.load(Ordering::Acquire);
    let deadline_ns = (now + ticks) * 10_000_000;
    pt_start(proc, SYS_WAIT_ANY, [ADMISSION_KILL_VA, 1, deadline_ns, 0], 0)
}

#[cfg(feature = "neo-test-pipe-core")]
fn run_pipe_core() -> ! {
    use crate::errno::{EAGAIN, EINVAL, EPERM, EPIPE};
    use crate::ipc::pipe::{PipeObject, race};
    use crate::ipc::pipe_logic::End;
    use crate::sched::wait_any::{self, REBIND_DONE, REBIND_IN_WINDOW, REBIND_PID, STATS};
    use alloc::sync::Arc;
    if crate::arch::x86_64::per_cpu::current_cpu_id() != 0 {
        fail("reason=test_not_on_bsp");
    }
    let new_pipe = |cap: usize| PipeObject::new(cap).unwrap_or_else(|_| fail("reason=pipe_new"));

    // --- create: SYS_PIPE_CREATE installs both ends in the caller; a write
    // and a read on them round-trip; capacity 0, and 511 (below
    // PIPE_ATOMIC), are EINVAL; the read end can't write.
    {
        let (proc, base) = pt_process();
        let pmap = proc.pmap.clone().expect("stand-in pmap");
        let pid = proc.pid;
        drop(proc);
        // SAFETY: the stand-in's own page, before its thread runs.
        unsafe { core::ptr::copy_nonoverlapping(b"abc".as_ptr(), base.add(PT_BUF as usize), 3) };
        PT_CREATE_DONE.store(false, Ordering::Release);
        let mut thread = crate::sched::thread::Thread::new("pipe-create", pt_create_caller);
        thread.bind_process_for_test(pid, pmap);
        if crate::sched::scheduler::place_thread_on_cpu(thread, 0).is_err() {
            fail("reason=pipe_place_thread_failed");
        }
        pt_until("reason=pipe_create_timeout", || {
            PT_CREATE_DONE.load(Ordering::Acquire) && pt_gone(pid.0)
        });
        let [bad, small, both, w, r, wrong, closed, eof] =
            crate::arch::x86_64::without_interrupts(|| *PT_CREATE.lock());
        // SAFETY: the stand-in has exited; its page is no longer written.
        let got = unsafe { core::slice::from_raw_parts(base.add(PT_BUF as usize + 0x80), 3) };
        serial_println!(
            "PIPE create bad_capacity={} small_capacity={} handles_ok={} write={} read={} bytes_ok={} read_end_write={} close_write={} read_after_close={}",
            bad,
            small,
            both > 0,
            w,
            r,
            got == b"abc",
            wrong,
            closed,
            eof
        );
        if closed != 0 || eof != 0 {
            fail("reason=pipe_close_syscall_wrong");
        }
        if bad != EINVAL || small != EINVAL || both <= 0 || w != 3 || r != 3 || got != b"abc" || wrong != EPERM {
            fail("reason=pipe_create_wrong");
        }
    }

    // --- round trip: a reader parks on an empty pipe; a writer's five bytes
    // wake it and arrive whole; with both gone, both ends are closed and only
    // the harness's `Arc` is left. The writer's handle is installed before
    // the reader starts (with no writer the read would be EOF at once); its
    // thread starts after the reader parks.
    {
        pt_reset();
        let pipe = new_pipe(64);
        let (wproc, wh) = {
            let (proc, base) = pt_process();
            // SAFETY: the stand-in's own page, before its thread runs.
            unsafe { core::ptr::copy_nonoverlapping(b"hello".as_ptr(), base.add(PT_BUF as usize), 5) };
            let h = pt_install(&proc, &pipe, End::Write);
            (proc, h)
        };
        let (rd, rbase) = pt_reader(&pipe, 16, 0);
        pt_until("reason=pipe_round_reader_never_parked", || {
            pipe.parked_for_test(End::Read) == 1
        });
        let wr = pt_start(
            wproc,
            crate::arch::x86_64::syscall::SYS_PIPE_WRITE,
            [wh as u64, crate::arch::x86_64::syscall::ADMISSION_KILL_VA + PT_BUF, 5, 0],
            0,
        );
        let r = pt_finish("reason=pipe_round_reader_stranded", rd);
        let w = pt_finish("reason=pipe_round_writer_stranded", wr);
        pt_settle();
        // SAFETY: the reader has exited.
        let got = unsafe { core::slice::from_raw_parts(rbase.add(PT_BUF as usize), 5) };
        let (nr, nw, buffered) = pipe.counts_for_test();
        let arc = Arc::strong_count(&pipe);
        serial_println!(
            "PIPE round read={} write={} bytes_ok={} counts={}/{}/{} arc={}",
            r,
            w,
            got == b"hello",
            nr,
            nw,
            buffered,
            arc
        );
        if r != 5 || w != 5 || got != b"hello" || (nr, nw, buffered) != (0, 0, 0) || arc != 1 {
            fail("reason=pipe_round_wrong");
        }
    }

    // --- EOF: a reader parks; the only writer makes a zero-length write
    // (0: a reader remains) and exits. Its last endpoint wakes the reader,
    // which reads EOF.
    {
        pt_reset();
        let pipe = new_pipe(64);
        let (wproc, _) = pt_process();
        let wh = pt_install(&wproc, &pipe, End::Write);
        let (rd, _) = pt_reader(&pipe, 16, 0);
        pt_until("reason=pipe_eof_reader_never_parked", || {
            pipe.parked_for_test(End::Read) == 1
        });
        let wr = pt_start(
            wproc,
            crate::arch::x86_64::syscall::SYS_PIPE_WRITE,
            [wh as u64, crate::arch::x86_64::syscall::ADMISSION_KILL_VA + PT_BUF, 0, 0],
            0,
        );
        let w = pt_finish("reason=pipe_eof_writer_stranded", wr);
        let r = pt_finish("reason=pipe_eof_reader_stranded", rd);
        serial_println!("PIPE eof zero_write={} read={}", w, r);
        if w != 0 || r != 0 {
            fail("reason=pipe_eof_wrong");
        }
    }

    // --- EPIPE: a 32-byte write into a 16-byte pipe stores 16 and parks for
    // the rest; the only reader makes a zero-length read (0) and exits. The
    // writer wakes and returns the 16 it wrote; a later write through a
    // write end the harness holds is EPIPE (through a closed write end it
    // would be EBADF, #56 F1).
    {
        pt_reset();
        let pipe = new_pipe(16);
        pipe.retain(End::Write);
        let (rd0, _) = {
            // The reader must exist before the write, or the write is EPIPE
            // at once: install it first, start it after the writer parks.
            let (proc, base) = pt_process();
            let h = pt_install(&proc, &pipe, End::Read);
            ((proc, h), base)
        };
        let wr = pt_writer(&pipe, 32, 0x77);
        pt_until("reason=pipe_epipe_writer_never_parked", || {
            pipe.parked_for_test(End::Write) == 1
        });
        let (proc, h) = rd0;
        let rd = pt_start(
            proc,
            crate::arch::x86_64::syscall::SYS_PIPE_READ,
            [h as u64, crate::arch::x86_64::syscall::ADMISSION_KILL_VA + PT_BUF, 0, 0],
            0,
        );
        let r = pt_finish("reason=pipe_epipe_reader_stranded", rd);
        let w = pt_finish("reason=pipe_epipe_writer_stranded", wr);
        let after = pipe.try_write(b"x");
        pipe.release(End::Write);
        serial_println!(
            "PIPE epipe zero_read={} partial_write={} later_write_epipe={}",
            r,
            w,
            after == Err(EPIPE)
        );
        if r != 0 || w != 16 || after != Err(EPIPE) {
            fail("reason=pipe_epipe_wrong");
        }
    }

    // --- atomic: a 512-byte pipe holds 300 bytes; a 300-byte write (atomic:
    // at most PIPE_ATOMIC) parks although the pipe is not full. A 100-byte
    // read frees enough room: it must wake the writer (every read that frees
    // space wakes writers), and the 300 bytes go in whole, after the rest.
    {
        pt_reset();
        let pipe = new_pipe(512);
        pipe.retain(End::Read);
        if pipe.try_write(&[0xaa; 300]) != Ok(300) {
            fail("reason=pipe_atomic_prefill");
        }
        let wr = pt_writer(&pipe, 300, 0x5b);
        pt_until("reason=pipe_atomic_writer_never_parked", || {
            pipe.parked_for_test(End::Write) == 1
        });
        let held = pipe.counts_for_test().2;
        let mut sink = [0u8; 512];
        if pipe.try_read(&mut sink[..100]) != Ok(100) {
            fail("reason=pipe_atomic_read");
        }
        let w = pt_finish("reason=pipe_atomic_writer_stranded", wr);
        let n = pipe.try_read(&mut sink).unwrap_or(0);
        let order_ok = n == 500
            && sink[..200].iter().all(|&b| b == 0xaa)
            && sink[200..500].iter().all(|&b| b == 0x5b);
        pipe.release(End::Read);
        serial_println!(
            "PIPE atomic held_while_parked={} write={} drained={} order_ok={}",
            held,
            w,
            n,
            order_ok
        );
        if held != 300 || w != 300 || !order_ok {
            fail("reason=pipe_atomic_wrong");
        }
    }

    // --- non-blocking: an empty pipe with a writer is EAGAIN at once.
    {
        pt_reset();
        let pipe = new_pipe(64);
        pipe.retain(End::Write);
        let (rd, _) = pt_reader(&pipe, 8, crate::arch::x86_64::syscall::PIPE_NONBLOCK);
        let r = pt_finish("reason=pipe_nonblock_stranded", rd);
        pipe.release(End::Write);
        serial_println!("PIPE nonblock read={}", r);
        if r != EAGAIN {
            fail("reason=pipe_nonblock_wrong");
        }
    }

    // --- two write handles (counts per handle, OT-88): a reader parks;
    // killing the first writer's process leaves it parked; killing the
    // second's ends it with EOF. Each writer is parked in a read of a
    // second pipe, so it has a thread to kill.
    {
        pt_reset();
        let pipe = new_pipe(64);
        let hold = new_pipe(16);
        hold.retain(End::Write);
        // The writers first: with no writer, the read would be EOF at once.
        let mut writers = [0u32; 2];
        for w in writers.iter_mut() {
            let (proc, _) = pt_process();
            pt_install(&proc, &pipe, End::Write);
            let h = pt_install(&proc, &hold, End::Read);
            *w = pt_start(
                proc,
                crate::arch::x86_64::syscall::SYS_PIPE_READ,
                [h as u64, crate::arch::x86_64::syscall::ADMISSION_KILL_VA + PT_BUF, 4, 0],
                0,
            );
        }
        pt_until("reason=pipe_two_writers_never_parked", || {
            hold.parked_for_test(End::Read) == 2
        });
        let (rd, _) = pt_reader(&pipe, 16, 0);
        pt_until("reason=pipe_two_reader_never_parked", || {
            pipe.parked_for_test(End::Read) == 1
        });
        let first = crate::sched::process::terminate(crate::sched::process::ProcessId(writers[0]));
        pt_until("reason=pipe_two_first_not_gone", || pt_gone(writers[0]));
        pt_settle();
        let still_parked = pt_result(rd).is_none();
        let writers_left = pipe.counts_for_test().1;
        let second = crate::sched::process::terminate(crate::sched::process::ProcessId(writers[1]));
        let r = pt_finish("reason=pipe_two_reader_stranded", rd);
        pt_until("reason=pipe_two_second_not_gone", || pt_gone(writers[1]));
        hold.release(End::Write);
        pt_settle();
        serial_println!(
            "PIPE two_writers kill1={} still_parked={} writers_left={} kill2={} read={} arc={}",
            first,
            still_parked,
            writers_left,
            second,
            r,
            Arc::strong_count(&pipe)
        );
        if first != 0 || second != 0 || !still_parked || writers_left != 1 || r != 0 {
            fail("reason=pipe_two_writers_wrong");
        }
        if Arc::strong_count(&pipe) != 1 || Arc::strong_count(&hold) != 1 {
            fail("reason=pipe_two_writers_arc");
        }
    }

    // --- WAIT_ANY, end-aware: a wait on a read end of an empty pipe parks
    // and a write ends it; a wait on a write end of a full pipe parks and a
    // read ends it. Each returns mask 1 well before its 300-tick deadline.
    // A write end is ready only when an atomic write would fit (Dexter #56
    // F2): on a full 16-byte pipe, a 1-byte read wakes the waiter, which
    // re-checks and parks again; draining the rest ends the wait.
    for (name, end) in [("read", End::Read), ("write", End::Write)] {
        pt_reset();
        let pipe = new_pipe(16);
        let other = match end {
            End::Read => End::Write,
            End::Write => End::Read,
        };
        pipe.retain(other);
        if end == End::Write && pipe.try_write(&[1; 16]) != Ok(16) {
            fail("reason=pipe_wait_prefill");
        }
        let (proc, base) = pt_process();
        let h = pt_install(&proc, &pipe, end);
        let pid = proc.pid;
        let parks0 = STATS.parks.load(Ordering::Relaxed);
        let waiter = pt_wait_any(proc, base, h, 300);
        pt_until("reason=pipe_wait_never_parked", || {
            STATS.parks.load(Ordering::Relaxed) > parks0 && wait_any::debug_entries(Some(pid)) == 1
        });
        // The write end: one byte of room is not enough.
        let mut early = false;
        if end == End::Write {
            let parks1 = STATS.parks.load(Ordering::Relaxed);
            let _ = pipe.try_read(&mut [0u8; 1]);
            pt_until("reason=pipe_wait_write_never_reparked", || {
                pt_result(waiter).is_some()
                    || (STATS.parks.load(Ordering::Relaxed) > parks1
                        && wait_any::debug_entries(Some(pid)) == 1)
            });
            early = pt_result(waiter).is_some();
        }
        let t0 = crate::arch::x86_64::timer::TICKS.load(Ordering::Acquire);
        match end {
            End::Read => {
                let _ = pipe.try_write(b"w");
            }
            End::Write => {
                let _ = pipe.try_read(&mut [0u8; 16]);
            }
        }
        let r = pt_finish("reason=pipe_wait_stranded", waiter);
        let latency = crate::arch::x86_64::timer::TICKS
            .load(Ordering::Acquire)
            .saturating_sub(t0);
        pipe.release(other);
        serial_println!(
            "PIPE wait_{} early={} result={} latency_ticks={}",
            name,
            early,
            r,
            latency
        );
        if early || r != 1 || latency > 100 {
            fail("reason=pipe_wait_any_wrong");
        }
    }

    // --- a sibling closes the handle a thread is parked on (Dexter #56
    // F1): the last read end closed under a parked reader, and the last
    // write end under a parked writer. The close wakes the parked thread,
    // and its retry is EBADF instead of a park nothing can end. The harness
    // closes the handle as `SYS_CLOSE` does (`HandleTable::remove`).
    for (name, end) in [("read", End::Read), ("write", End::Write)] {
        pt_reset();
        let pipe = new_pipe(16);
        let other = match end {
            End::Read => End::Write,
            End::Write => End::Read,
        };
        pipe.retain(other);
        if end == End::Write && pipe.try_write(&[1; 16]) != Ok(16) {
            fail("reason=pipe_close_prefill");
        }
        let (proc, _) = pt_process();
        let h = pt_install(&proc, &pipe, end);
        let pid = proc.pid;
        let num = match end {
            End::Read => crate::arch::x86_64::syscall::SYS_PIPE_READ,
            End::Write => crate::arch::x86_64::syscall::SYS_PIPE_WRITE,
        };
        let caller = pt_start(
            proc,
            num,
            [h as u64, crate::arch::x86_64::syscall::ADMISSION_KILL_VA + PT_BUF, 8, 0],
            0,
        );
        pt_until("reason=pipe_close_never_parked", || {
            pipe.parked_for_test(end) == 1
        });
        let gone = crate::sched::process::get(pid).and_then(|p| {
            crate::arch::x86_64::without_interrupts(|| p.handles.lock().remove(h))
        });
        let closed = gone.is_some();
        // The endpoint is released here, outside the handle lock.
        drop(gone);
        let r = pt_finish("reason=pipe_close_parked_stranded", caller);
        pipe.release(other);
        pt_settle();
        serial_println!(
            "PIPE close_parked_{} closed={} result={} arc={}",
            name,
            closed,
            r,
            Arc::strong_count(&pipe)
        );
        if !closed || r != crate::errno::EBADF || Arc::strong_count(&pipe) != 1 {
            fail("reason=pipe_close_parked_wrong");
        }
    }

    // --- CP-61, rebind: while the waiter is between its first scan and its
    // registration, a sibling closes its read end of pipe A and installs a
    // read end of pipe B in the same slot. A write into B must end the wait
    // (its second scan sees another identity and re-watches).
    // `wait-multiplex-no-identity-check`: it watches A, which nothing fires,
    // and only its deadline ends it (result 0).
    {
        pt_reset();
        let a = new_pipe(16);
        let b = new_pipe(16);
        a.retain(End::Write);
        b.retain(End::Write);
        *PT_REBIND_PIPE.lock() = Some(b.clone());
        let (proc, base) = pt_process();
        let h = pt_install(&proc, &a, End::Read);
        PT_REBIND_HANDLE.store(h, Ordering::Release);
        let pid = proc.pid;
        let pmap = proc.pmap.clone().expect("stand-in pmap");
        let retries0 = STATS.identity_retries.load(Ordering::Relaxed);
        let parks0 = STATS.parks.load(Ordering::Relaxed);
        REBIND_DONE.store(false, Ordering::Release);
        REBIND_IN_WINDOW.store(false, Ordering::Release);
        REBIND_PID.store(pid.0, Ordering::Release);
        let mut sibling = crate::sched::thread::Thread::new("pipe-rebind", pt_rebind_sibling);
        sibling.bind_process_for_test(pid, pmap);
        if crate::sched::scheduler::place_thread_on_cpu(sibling, 1).is_err() {
            fail("reason=pipe_place_sibling_failed");
        }
        let waiter = pt_wait_any(proc, base, h, 300);
        pt_until("reason=pipe_rebind_never_parked", || {
            REBIND_DONE.load(Ordering::Acquire)
                && (STATS.parks.load(Ordering::Relaxed) > parks0 || pt_result(waiter).is_some())
        });
        if PT_REBIND_SLOT_MOVED.load(Ordering::Acquire) {
            fail("reason=pipe_rebind_slot_not_reused");
        }
        let t0 = crate::arch::x86_64::timer::TICKS.load(Ordering::Acquire);
        let _ = b.try_write(b"b");
        pt_until("reason=pipe_rebind_waiter_stranded", || {
            pt_result(waiter).is_some()
        });
        let r = pt_result(waiter).unwrap_or(i64::MIN);
        let latency = crate::arch::x86_64::timer::TICKS
            .load(Ordering::Acquire)
            .saturating_sub(t0);
        let retries = STATS.identity_retries.load(Ordering::Relaxed) - retries0;
        let rc = crate::sched::process::terminate(pid);
        pt_until("reason=pipe_rebind_not_gone", || pt_gone(pid.0));
        *PT_REBIND_PIPE.lock() = None;
        a.release(End::Write);
        b.release(End::Write);
        pt_settle();
        serial_println!(
            "PIPE rebind result={} latency_ticks={} identity_retries={} kill={} arc={}/{}",
            r,
            latency,
            retries,
            rc,
            Arc::strong_count(&a),
            Arc::strong_count(&b)
        );
        if r != 1 || latency > 100 || retries == 0 {
            fail("reason=pipe_rebind_stranded");
        }
        if Arc::strong_count(&a) != 1 || Arc::strong_count(&b) != 1 {
            fail("reason=pipe_rebind_arc");
        }
    }

    // --- CP-14: a write from CPU 1 lands while the reader is between its
    // empty check and its park (the window held open). The reader must wake
    // every round. `neo-test-pipe-core-cp14-control` drops the guard across
    // the window: the write finds no token, the reader parks on a pipe that
    // has a byte, and the round fails on its deadline.
    const RACE_ROUNDS: u64 = 8;
    {
        pt_reset();
        let hits0 = race::HITS.load(Ordering::Relaxed);
        PT_RACE_WROTE.store(0, Ordering::Release);
        PT_RACE_DONE.store(0, Ordering::Release);
        for round in 0..RACE_ROUNDS {
            let pipe = new_pipe(64);
            pipe.retain(End::Write);
            *PT_RACE_PIPE.lock() = Some(pipe.clone());
            race::IN_WINDOW.store(false, Ordering::Release);
            race::WRITER_STARTED.store(false, Ordering::Release);
            race::ARMED.store(true, Ordering::Release);
            let writer = crate::sched::thread::Thread::new("pipe-race-writer", pt_race_writer);
            if crate::sched::scheduler::place_thread_on_cpu(writer, 1).is_err() {
                fail("reason=pipe_place_writer_failed");
            }
            let (rd, _) = pt_reader(&pipe, 4, 0);
            pt_until("reason=pipe_cp14_lost_wake", || pt_result(rd).is_some());
            let r = pt_result(rd).unwrap_or(i64::MIN);
            pt_until("reason=pipe_cp14_writer_stranded", || {
                PT_RACE_DONE.load(Ordering::Acquire) == round + 1 && pt_gone(rd)
            });
            *PT_RACE_PIPE.lock() = None;
            pipe.release(End::Write);
            if r != 1 {
                fail("reason=pipe_cp14_wrong_read");
            }
        }
        pt_settle();
        let hits = race::HITS.load(Ordering::Relaxed) - hits0;
        serial_println!(
            "PIPE cp14 rounds={} woken={} window_hits={}",
            RACE_ROUNDS,
            PT_RACE_WROTE.load(Ordering::Acquire),
            hits
        );
        if PT_RACE_WROTE.load(Ordering::Acquire) != RACE_ROUNDS || hits == 0 {
            fail("reason=pipe_cp14_window_not_hit");
        }
    }

    let zombies = crate::sched::scheduler::debug_total_zombies();
    if zombies != 0 {
        fail("reason=pipe_core_zombies_left");
    }
    pass(
        "pipe_core=ok create=round_trip round=5 eof=0 epipe=partial_then_epipe atomic=whole nonblock=eagain two_writers=eof_on_last wait=read,write_atomic close_parked=ebadf rebind=rewatched cp14=no_lost_wake",
    )
}

// ── pipe_park_kill ──

/// CP-33 / CP-67: threads killed while parked in a pipe read and in a pipe
/// write leave no pipe count behind (the control keeps the pipe on the
/// syscall frame across the park: the kill leaks it). A killed thread's
/// token left in the waiter list is harmless to the next reader; a list full
/// of killed threads' tokens is purged, and one full of live waiters is
/// EAGAIN.
#[cfg(feature = "neo-test-pipe-park-kill")]
fn run_pipe_park_kill() -> ! {
    use crate::errno::EAGAIN;
    use crate::ipc::pipe::{PARK_MAX, PipeObject};
    use crate::ipc::pipe_logic::End;
    use crate::sched::process::{ProcessId, terminate};
    use alloc::sync::Arc;
    if crate::arch::x86_64::per_cpu::current_cpu_id() != 0 {
        fail("reason=test_not_on_bsp");
    }
    let new_pipe = |cap: usize| PipeObject::new(cap).unwrap_or_else(|_| fail("reason=pipe_new"));
    let mut leaked: Option<&str> = None;

    // --- a reader killed while parked. The harness holds both ends, so the
    // pipe stays open for the stale-token leg.
    let pipe = new_pipe(64);
    pipe.retain(End::Read);
    pipe.retain(End::Write);
    {
        pt_reset();
        let (rd, _) = pt_reader(&pipe, 16, 0);
        pt_until("reason=pipe_kill_reader_never_parked", || {
            pipe.parked_for_test(End::Read) == 1
        });
        let parked = Arc::strong_count(&pipe);
        let rc = terminate(ProcessId(rd));
        pt_until("reason=pipe_kill_reader_not_gone", || pt_gone(rd));
        pt_settle();
        let after = Arc::strong_count(&pipe);
        serial_println!(
            "PIPE kill leg=read terminate={} returned={:?} arc parked={} after={} readers={}",
            rc,
            pt_result(rd),
            parked,
            after,
            pipe.counts_for_test().0
        );
        if rc != 0 || pt_result(rd).is_some() {
            fail("reason=pipe_kill_victim_not_killed");
        }
        if after != 1 || pipe.counts_for_test().0 != 1 {
            leaked.get_or_insert("read");
        }
    }

    // --- the stale token: the killed reader's token is still in the list.
    // A new reader parks beside it; a write wakes both (the stale one is
    // `Gone`) and the new reader gets the byte.
    {
        pt_reset();
        let stale = pipe.parked_for_test(End::Read);
        let (rd, _) = pt_reader(&pipe, 16, 0);
        pt_until("reason=pipe_stale_reader_never_parked", || {
            pipe.parked_for_test(End::Read) == stale + 1
        });
        let _ = pipe.try_write(b"q");
        let r = pt_finish("reason=pipe_stale_reader_stranded", rd);
        serial_println!("PIPE stale tokens_before={} read={}", stale, r);
        if stale != 1 || r != 1 {
            fail("reason=pipe_stale_token_wrong");
        }
    }
    pipe.release(End::Read);
    pipe.release(End::Write);

    // --- a writer killed while parked on a full pipe.
    {
        pt_reset();
        let pipe = new_pipe(16);
        pipe.retain(End::Read);
        if pipe.try_write(&[3; 16]) != Ok(16) {
            fail("reason=pipe_kill_prefill");
        }
        let wr = pt_writer(&pipe, 8, 0x44);
        pt_until("reason=pipe_kill_writer_never_parked", || {
            pipe.parked_for_test(End::Write) == 1
        });
        let parked = Arc::strong_count(&pipe);
        let rc = terminate(ProcessId(wr));
        pt_until("reason=pipe_kill_writer_not_gone", || pt_gone(wr));
        pt_settle();
        let after = Arc::strong_count(&pipe);
        let (_, writers, buffered) = pipe.counts_for_test();
        pipe.release(End::Read);
        serial_println!(
            "PIPE kill leg=write terminate={} returned={:?} arc parked={} after={} writers={} buffered={}",
            rc,
            pt_result(wr),
            parked,
            after,
            writers,
            buffered
        );
        if rc != 0 || pt_result(wr).is_some() {
            fail("reason=pipe_kill_victim_not_killed");
        }
        if after != 1 || writers != 0 {
            leaked.get_or_insert("write");
        }
    }
    if let Some(name) = leaked {
        fail(&alloc::format!("reason=pipe_park_kill_arc_leaked leg={name}"));
    }

    // --- a full list: PARK_MAX readers parked, then killed, leave PARK_MAX
    // stale tokens. The next reader purges them and parks. With PARK_MAX
    // live readers parked, one more is EAGAIN at once. Closing the write end
    // ends them all (EOF), and the one write before it reaches one of them.
    {
        pt_reset();
        let pipe = new_pipe(64);
        pipe.retain(End::Read);
        pipe.retain(End::Write);
        let mut victims = [0u32; PARK_MAX];
        for v in victims.iter_mut() {
            *v = pt_reader(&pipe, 16, 0).0;
        }
        pt_until("reason=pipe_full_victims_never_parked", || {
            pipe.parked_for_test(End::Read) == PARK_MAX
        });
        for v in victims {
            let _ = terminate(ProcessId(v));
        }
        pt_until("reason=pipe_full_victims_not_gone", || {
            victims.iter().all(|&v| pt_gone(v))
        });
        pt_settle();
        let stale = pipe.parked_for_test(End::Read);
        pt_reset();
        let mut live = [0u32; PARK_MAX];
        for l in live.iter_mut() {
            *l = pt_reader(&pipe, 16, 0).0;
        }
        pt_until("reason=pipe_full_live_never_parked", || {
            pipe.parked_for_test(End::Read) == PARK_MAX
        });
        let (extra, _) = pt_reader(&pipe, 16, 0);
        let extra_r = pt_finish("reason=pipe_full_extra_stranded", extra);
        let _ = pipe.try_write(&[9; 8]);
        pipe.release(End::Write);
        pt_until("reason=pipe_full_live_stranded", || {
            live.iter().all(|&l| pt_result(l).is_some() && pt_gone(l))
        });
        let total: i64 = live.iter().map(|&l| pt_result(l).unwrap_or(-1000)).sum();
        pipe.release(End::Read);
        pt_settle();
        serial_println!(
            "PIPE full stale_before={} extra={} live_total={} arc={}",
            stale,
            extra_r,
            total,
            Arc::strong_count(&pipe)
        );
        if stale != PARK_MAX || extra_r != EAGAIN || total != 8 || Arc::strong_count(&pipe) != 1 {
            fail("reason=pipe_full_list_wrong");
        }
    }

    let zombies = crate::sched::scheduler::debug_total_zombies();
    if zombies != 0 {
        fail("reason=pipe_park_kill_zombies_left");
    }
    pass("pipe_park_kill=ok legs=read,write arc=1 stale=harmless full=purged_then_eagain")
}
