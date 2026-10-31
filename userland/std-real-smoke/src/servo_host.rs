// SPDX-License-Identifier: MPL-2.0
//! OT-61 C2.3a — `servo-host-smoke`, the C2 rung's closing smoke (ADR-078 §3;
//! `docs/RESEARCH-2026-07-22-ot61-servo-host-closure-spike-FINDINGS.md`, "The
//! first prototype"), without the third-party crates (html5ever and Stylo
//! are C2.3b).
//!
//! The kernel spawns this binary twice, side by side, each under its own
//! memory budget (`mem-quota`), and it picks its role from what it holds:
//!
//! - **The browser-runtime cap (the smoke):** it sleeps first, so the
//!   sibling's death happens while it is alive, then checks threads,
//!   thread-locals, `Mutex` + `Condvar` on the monotonic clock, the heap, the
//!   sealed resource, and the three ambient-authority refusals (root open,
//!   environment enumeration, network dial). Exit 0. The sealed resource, a
//!   read-only file, is installed by the kernel just after the spawn, as a
//!   broker would hand it over (an install keeps files off the stdio
//!   indices, which spawn inheritance does not), so the smoke waits for it.
//! - **Nothing (the content OOM):** it grows a `Vec` until its budget refuses
//!   a page. std's allocation-failure handler aborts: exit 134, and only this
//!   process ends.
//!
//! A failed check prints `FAIL(n)` and exits with `n`.

#![no_main]
#![feature(restricted_std)]
#![feature(neoplutarch_ext)]

use std::cell::{Cell, RefCell};
use std::fs::File;
use std::io::{ErrorKind, Read};
use std::net::{SocketAddr, TcpStream, UdpSocket};
use std::os::neoplutarch::io::FromRawHandle;
use std::os::neoplutarch::{HandleKind, handles};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use neo_std_pal::SyscallHeap;

#[global_allocator]
static HEAP: SyscallHeap = SyscallHeap::new();

const TAG: &str = "[ servo-host ]";
const FILE_READ: u32 = 1 << 0;
const FILE_WRITE: u32 = 1 << 1;
/// The kernel hands this process `/hello.txt` on its root testfs.
const SEALED: &[u8] = b"NeoPlutarch testfs hello\n";
/// How long the smoke sleeps before its checks, so the OOM sibling dies
/// while it runs.
const HEAD_START: Duration = Duration::from_millis(500);

fn check(code: i32, what: &str, ok: bool) {
    if ok {
        println!("{TAG} ok: {what}");
    } else {
        println!("{TAG} FAIL({code}): {what}");
        std::process::exit(code);
    }
}

thread_local! {
    static TL: Cell<u64> = const { Cell::new(0) };
    static PROBE: RefCell<Option<DropProbe>> = const { RefCell::new(None) };
}

/// Sum of the ids of dropped probes: a thread-local destructor ran.
static DROPPED: AtomicUsize = AtomicUsize::new(0);

struct DropProbe(usize);

impl Drop for DropProbe {
    fn drop(&mut self) {
        DROPPED.fetch_add(self.0, Ordering::SeqCst);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let held: Vec<_> = handles().collect();
    if held.is_empty() {
        oom();
    }
    smoke(&held);
}

/// The content OOM: grow until the budget says no. Never returns normally.
fn oom() -> ! {
    println!("{TAG} oom role: growing a Vec until the budget refuses");
    let mut chunks: Vec<Vec<u8>> = Vec::new();
    loop {
        let mut v = vec![0u8; 256 * 1024];
        // Touch every page, so the frames are really used.
        for i in (0..v.len()).step_by(4096) {
            v[i] = 1;
        }
        chunks.push(v);
    }
}

fn smoke(held: &[(std::os::neoplutarch::io::RawHandle, std::os::neoplutarch::HandleInfo)]) -> ! {
    let _ = held;
    // 1. Monotonic clock, and the head start.
    let t0 = Instant::now();
    thread::sleep(HEAD_START);
    check(1, "sleep: Instant moved at least the sleep", t0.elapsed() >= HEAD_START);

    // 2. Two native threads, spawned and joined.
    let a = thread::spawn(|| 40u64 + 1);
    let b = thread::spawn(|| 2u64 * 21);
    let (ra, rb) = (a.join(), b.join());
    check(2, "two threads spawn and join with their results", matches!((ra, rb), (Ok(41), Ok(42))));

    // 3. Per-thread TLS isolation, and destructors on thread exit.
    TL.with(|t| t.set(7));
    let workers: Vec<_> = (1..=2u64)
        .map(|id| {
            thread::spawn(move || {
                let fresh = TL.with(|t| t.get());
                TL.with(|t| t.set(100 + id));
                PROBE.with(|p| *p.borrow_mut() = Some(DropProbe(id as usize)));
                (fresh, TL.with(|t| t.get()))
            })
        })
        .collect();
    let seen: Vec<_> = workers.into_iter().map(|w| w.join()).collect();
    check(
        3,
        "thread-locals: each thread starts at 0 and keeps its own value",
        seen.iter().zip(1..=2u64).all(|(r, id)| matches!(r, Ok((0, v)) if *v == 100 + id))
            && TL.with(|t| t.get()) == 7,
    );
    check(4, "thread-local destructors ran on both threads' exit", DROPPED.load(Ordering::SeqCst) == 3);

    // 4. Mutex + Condvar: a wake, and a timeout on the monotonic clock.
    //    The waker sets the predicate only once the waiter is parked: the
    //    waiter marks itself `parked` under the mutex and releases it only by
    //    waiting, so a waker holding the mutex and seeing `parked` knows the
    //    wait has begun. The predicate is false when the wait starts, so the
    //    wait ends early only if `notify_one` wakes it. Without it, the wait
    //    runs to its 10 s timeout, and `wait_timeout_while` still reports "not
    //    timed out" (it re-checks the predicate, which the waker set), so the
    //    check bounds the time waited too.
    let pair = Arc::new((Mutex::new((false, false)), Condvar::new()));
    let waker = {
        let pair = Arc::clone(&pair);
        thread::spawn(move || {
            let (m, c) = &*pair;
            loop {
                let mut state = m.lock().unwrap();
                if state.0 {
                    state.1 = true;
                    drop(state);
                    c.notify_one();
                    return;
                }
                drop(state);
                thread::yield_now();
            }
        })
    };
    let (m, c) = &*pair;
    let woke = {
        let mut guard = m.lock().unwrap();
        guard.0 = true; // parked, from here until the wait releases the mutex
        let t0 = Instant::now();
        let (guard, r) = c.wait_timeout_while(guard, Duration::from_secs(10), |s| !s.1).unwrap();
        guard.1 && !r.timed_out() && t0.elapsed() < Duration::from_secs(5)
    };
    let _ = waker.join();
    check(5, "Condvar: notify wakes the waiter", woke);
    let start = Instant::now();
    let timed_out = {
        let guard = m.lock().unwrap();
        let (_g, r) = c.wait_timeout(guard, Duration::from_millis(30)).unwrap();
        r.timed_out()
    };
    check(
        6,
        "Condvar: an unnotified wait times out after its duration",
        timed_out && start.elapsed() >= Duration::from_millis(30),
    );

    // 5. Heap: allocate, free and regrow.
    let mut v: Vec<u64> = Vec::new();
    for i in 0..(256 * 1024) {
        v.push(i);
    }
    let sum: u64 = v.iter().sum();
    v.truncate(16);
    v.shrink_to_fit();
    drop(v);
    let mut w: Vec<u64> = Vec::with_capacity(512 * 1024);
    w.extend(0..512 * 1024);
    check(
        7,
        "heap: 2 MiB grown, freed, and 4 MiB grown again",
        sum == (256 * 1024 - 1) * (256 * 1024) / 2 && w.len() == 512 * 1024 && w[512 * 1024 - 1] == 512 * 1024 - 1,
    );
    drop(w);

    // 6. The sealed resource, supplied by handle: a read-only regular file,
    // and no RootDirCap. Wait (bounded) for the kernel's install.
    let readable = |(_, i): &(_, std::os::neoplutarch::HandleInfo)| {
        i.kind == HandleKind::File && i.file_rights & FILE_READ != 0
    };
    let mut held: Vec<_> = handles().collect();
    for _ in 0..500 {
        if held.iter().any(readable) {
            break;
        }
        thread::sleep(Duration::from_millis(10));
        held = handles().collect();
    }
    check(8, "no RootDirCap is held", held.iter().all(|(_, i)| !i.root_dir));
    check(
        9,
        "the browser-runtime cap is held",
        held.iter().any(|(_, i)| i.kind == HandleKind::BrowserRuntime),
    );
    let sealed = held.iter().find(|h| readable(h));
    let Some((raw, info)) = sealed else {
        println!("{TAG} FAIL(10): no readable file handle was handed in");
        std::process::exit(10);
    };
    check(11, "the sealed file is read-only", info.file_rights & FILE_WRITE == 0);
    // SAFETY: `raw` is this process's own file handle, owned by nothing else.
    let mut f = unsafe { File::from_raw_handle(*raw) };
    let mut body = Vec::new();
    let read = f.read_to_end(&mut body);
    println!("{TAG} sealed read: {read:?}, {} bytes: {:?}", body.len(), String::from_utf8_lossy(&body));
    check(12, "the sealed file reads its known bytes", read.is_ok() && body == SEALED);

    // 7. The three ambient-authority refusals.
    check(
        13,
        "refusal 1: an ambient root open is PermissionDenied",
        matches!(File::open("/hello.txt"), Err(e) if e.kind() == ErrorKind::PermissionDenied),
    );
    check(
        14,
        "refusal 2: the environment enumerates nothing",
        std::env::vars_os().next().is_none()
            && std::env::var_os("PATH").is_none()
            && std::env::var_os("HOME").is_none(),
    );
    let dial = TcpStream::connect(SocketAddr::from(([10, 0, 2, 2], 80)));
    let bind = UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0], 0)));
    check(
        15,
        "refusal 3: a network dial and a socket bind are refused",
        matches!(dial, Err(e) if e.kind() == ErrorKind::Unsupported)
            && matches!(bind, Err(e) if e.kind() == ErrorKind::Unsupported),
    );

    println!("{TAG} PASS: threads, TLS, Condvar, heap, the sealed file, and three refusals");
    std::process::exit(0);
}
