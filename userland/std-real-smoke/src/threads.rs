// SPDX-License-Identifier: MPL-2.0
//! OT-61 C1.3 — the **real-`std` threads** battery (`std_threads` scenario).
//!
//! The same overlay-built `std` as `std-real-smoke`, now with `std::thread`,
//! std's futex-based sync, `thread_local!` on the per-thread key layer and
//! `Instant` on the monotonic clock. The kernel spawns this binary twice:
//!
//! - **without** the browser-runtime capability: spawning a thread must fail
//!   with `PermissionDenied`, while thread-locals, `Mutex` and `Once` keep
//!   working on the initial thread's static key block. Exit 41.
//! - **with** the capability: every check below. Exit 0.
//!
//! A failed check prints `FAIL(n)` and exits with `n`, so the kernel log's exit
//! code localizes it. Thread counts stay under the kernel's 16 threads per
//! process.

#![no_main]
#![feature(restricted_std)]

use std::cell::{Cell, RefCell};
use std::io::ErrorKind;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Condvar, Mutex, OnceLock, RwLock, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use neo_std_pal::SyscallHeap;

#[global_allocator]
static HEAP: SyscallHeap = SyscallHeap::new();

const TAG: &str = "[ std-threads ]";

/// Exit code of the no-capability role.
const NO_CAP_EXIT: i32 = 41;

/// Threads per concurrent check (with the main thread, well under 16).
const N: usize = 4;

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
    // 1. Before any thread exists: the initial thread's keys, on the static
    //    block (no FS.base yet), and the uncontended futex fast paths.
    TL.with(|t| t.set(7));
    check(
        1,
        "thread_local! on the initial thread",
        TL.with(Cell::get) == 7,
    );
    let m = Mutex::new(0u32);
    *m.lock().unwrap() += 1;
    let once = OnceLock::new();
    check(
        2,
        "Mutex + OnceLock uncontended",
        *m.lock().unwrap() == 1 && *once.get_or_init(|| 3) == 3,
    );

    // 3. The role: a process without the browser-runtime capability.
    match thread::Builder::new().spawn(|| 5u32) {
        Err(e) if e.kind() == ErrorKind::PermissionDenied => {
            check(
                3,
                "no capability: thread spawn -> PermissionDenied; keys still work",
                TL.with(Cell::get) == 7,
            );
            println!("{TAG} PASS (no-cap role): spawn refused, single-threaded std intact");
            std::process::exit(NO_CAP_EXIT);
        }
        Err(e) => {
            println!("{TAG} FAIL(4): thread spawn failed: {e:?}");
            std::process::exit(4);
        }
        Ok(h) => check(
            5,
            "spawn + join returns the closure's value",
            h.join().ok() == Some(5),
        ),
    }
    check(
        6,
        "the initial thread's value survived the switch to FS.base",
        TL.with(Cell::get) == 7,
    );

    // 7. thread_local! isolation across N threads that yield and sleep while
    //    holding distinct values.
    let start = Arc::new(Barrier::new(N));
    let handles: Vec<_> = (0..N)
        .map(|i| {
            let start = start.clone();
            thread::spawn(move || {
                let mine = 100 + i as u64;
                TL.with(|t| t.set(mine));
                start.wait();
                for _ in 0..20 {
                    thread::yield_now();
                    if TL.with(Cell::get) != mine {
                        return false;
                    }
                }
                thread::sleep(Duration::from_millis(20));
                TL.with(Cell::get) == mine
            })
        })
        .collect();
    let isolated = handles.into_iter().all(|h| h.join().unwrap_or(false));
    check(
        7,
        "thread_local! isolation: 4 threads, distinct values across switches",
        isolated,
    );
    check(
        8,
        "the initial thread's value is untouched",
        TL.with(Cell::get) == 7,
    );

    // 9. Thread-local destructors run at thread exit (std's key-path cleanup,
    //    driven by the key layer's destructor passes).
    DROPPED.store(0, Ordering::SeqCst);
    let handles: Vec<_> = (1..=N)
        .map(|id| thread::spawn(move || PROBE.with(|p| *p.borrow_mut() = Some(DropProbe(id)))))
        .collect();
    for h in handles {
        let _ = h.join();
    }
    check(
        9,
        "thread_local! destructors ran on every exiting thread",
        DROPPED.load(Ordering::SeqCst) == (1..=N).sum::<usize>(),
    );

    // 10. Mutex under contention (the futex wait/wake path).
    let counter = Arc::new(Mutex::new(0u64));
    let handles: Vec<_> = (0..N)
        .map(|_| {
            let counter = counter.clone();
            thread::spawn(move || {
                for i in 0..2_000 {
                    let mut c = counter.lock().unwrap();
                    *c += 1;
                    if i % 256 == 0 {
                        // Hold the lock across a yield so waiters really park.
                        thread::yield_now();
                    }
                }
            })
        })
        .collect();
    for h in handles {
        let _ = h.join();
    }
    check(
        10,
        "Mutex: 4 x 2000 contended increments",
        *counter.lock().unwrap() == 8_000,
    );

    // 11. Condvar: a handshake, then a timed wait that must time out.
    let pair = Arc::new((Mutex::new(false), Condvar::new()));
    let p2 = pair.clone();
    let signaller = thread::spawn(move || {
        thread::sleep(Duration::from_millis(30));
        *p2.0.lock().unwrap() = true;
        p2.1.notify_one();
    });
    let (lock, cvar) = &*pair;
    let ready = cvar.wait_while(lock.lock().unwrap(), |r| !*r).unwrap();
    check(11, "Condvar: wait_while woken by notify_one", *ready);
    drop(ready);
    let _ = signaller.join();
    let t0 = Instant::now();
    let (_g, res) = cvar
        .wait_timeout(lock.lock().unwrap(), Duration::from_millis(50))
        .unwrap();
    let waited = t0.elapsed();
    println!("{TAG} wait_timeout(50 ms) waited {waited:?}");
    check(
        12,
        "Condvar: wait_timeout(50 ms) times out after at least 50 ms",
        res.timed_out() && waited >= Duration::from_millis(50),
    );
    drop(_g);

    // 13. RwLock: concurrent readers, then a writer.
    let rw = Arc::new(RwLock::new(5u32));
    let readers: Vec<_> = (0..N)
        .map(|_| {
            let rw = rw.clone();
            thread::spawn(move || {
                let g = rw.read().unwrap();
                thread::sleep(Duration::from_millis(10));
                *g
            })
        })
        .collect();
    let read_ok = readers.into_iter().all(|h| h.join().ok() == Some(5));
    *rw.write().unwrap() += 1;
    check(
        13,
        "RwLock: 4 readers, then a writer",
        read_ok && *rw.read().unwrap() == 6,
    );

    // 14. OnceLock raced by N threads: the initializer runs once.
    static INITS: AtomicUsize = AtomicUsize::new(0);
    static CELL: OnceLock<usize> = OnceLock::new();
    let start = Arc::new(Barrier::new(N));
    let handles: Vec<_> = (0..N)
        .map(|i| {
            let start = start.clone();
            thread::spawn(move || {
                start.wait();
                *CELL.get_or_init(|| {
                    INITS.fetch_add(1, Ordering::SeqCst);
                    thread::sleep(Duration::from_millis(20));
                    i
                })
            })
        })
        .collect();
    let seen: Vec<usize> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    check(
        14,
        "OnceLock: one initializer, every thread sees its value",
        INITS.load(Ordering::SeqCst) == 1 && seen.iter().all(|&v| v == seen[0]),
    );

    // 15. mpsc: N producers, one consumer.
    let (tx, rx) = mpsc::channel::<u64>();
    let producers: Vec<_> = (0..N as u64)
        .map(|p| {
            let tx = tx.clone();
            thread::spawn(move || {
                for k in 0..100 {
                    tx.send(p * 1000 + k).unwrap();
                }
            })
        })
        .collect();
    drop(tx);
    let got: Vec<u64> = rx.iter().collect();
    for h in producers {
        let _ = h.join();
    }
    let want: u64 = (0..N as u64)
        .map(|p| (0..100).map(|k| p * 1000 + k).sum::<u64>())
        .sum();
    check(
        15,
        "mpsc: 4 producers x 100 messages",
        got.len() == 400 && got.iter().sum::<u64>() == want,
    );

    // 16. park/unpark and park_timeout.
    let flag = Arc::new(AtomicBool::new(false));
    let f2 = flag.clone();
    let parker = thread::spawn(move || {
        while !f2.load(Ordering::Acquire) {
            thread::park();
        }
        let t0 = Instant::now();
        thread::park_timeout(Duration::from_millis(30));
        t0.elapsed()
    });
    thread::sleep(Duration::from_millis(20));
    flag.store(true, Ordering::Release);
    parker.thread().unpark();
    let timed = parker.join().unwrap_or_default();
    // park_timeout may return early on a spurious or stale unpark token; it
    // must still return.
    check(
        16,
        "park/unpark, then park_timeout returns",
        timed <= Duration::from_secs(5),
    );

    // 17. sleep + Instant: monotonic, and sleep waits at least its duration.
    let t0 = Instant::now();
    thread::sleep(Duration::from_millis(60));
    let slept = t0.elapsed();
    let t1 = Instant::now();
    println!("{TAG} sleep(60 ms) took {slept:?}");
    check(
        17,
        "sleep(60 ms) >= 60 ms; Instant nondecreasing",
        slept >= Duration::from_millis(60) && t1 >= t0 + slept,
    );

    // 18. thread names and ids.
    let named = thread::Builder::new()
        .name("neo-std-worker".into())
        .spawn(|| {
            (
                thread::current().name().map(String::from),
                thread::current().id(),
            )
        })
        .unwrap()
        .join()
        .unwrap();
    check(
        18,
        "a named thread sees its name; its id differs from the initial thread's",
        named.0.as_deref() == Some("neo-std-worker") && named.1 != thread::current().id(),
    );

    // 19. Scoped threads borrowing the caller's data.
    let data = [1u64, 2, 3, 4, 5, 6, 7, 8];
    let total = AtomicUsize::new(0);
    thread::scope(|s| {
        for chunk in data.chunks(2) {
            let total = &total;
            s.spawn(move || {
                total.fetch_add(chunk.iter().sum::<u64>() as usize, Ordering::SeqCst);
            });
        }
    });
    check(
        19,
        "thread::scope: 4 scoped threads",
        total.load(Ordering::SeqCst) == 36,
    );

    // 20. A detached thread (its handle dropped) runs to completion, and 40
    //     sequential spawn+join cycles reuse the kernel's thread records.
    let done = Arc::new(AtomicBool::new(false));
    let d2 = done.clone();
    drop(thread::spawn(move || d2.store(true, Ordering::Release)));
    let t0 = Instant::now();
    while !done.load(Ordering::Acquire) && t0.elapsed() < Duration::from_secs(5) {
        thread::sleep(Duration::from_millis(5));
    }
    let cycles = (0..40).all(|i| thread::spawn(move || i * 2).join().ok() == Some(i * 2));
    check(
        20,
        "a detached thread ran; 40 spawn+join cycles",
        done.load(Ordering::Acquire) && cycles,
    );

    println!(
        "{TAG} PASS: std::thread + futex Mutex/Condvar/RwLock/Once/park + thread_local! (keys, destructors) + Instant/sleep"
    );
    std::process::exit(0);
}
