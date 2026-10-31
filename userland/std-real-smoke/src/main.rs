// SPDX-License-Identifier: MPL-2.0
//! OT-61 browser C0.5 — the **real-`std`-linked** smoke (the runtime close of
//! the C0 gate).
//!
//! Unlike the C0 `std-pal-smoke` (which stood directly on `neo-std-pal`'s raw
//! syscalls), this binary uses **actual `std`**, compiled with
//! `-Z build-std=std,panic_abort` against the `neoplutarch` rust-src overlay
//! (`userland/neoplutarch-std-overlay/`). It checks, in order:
//!
//! 1. **print** — `std::println!` / `eprintln!` reach serial (overlay stdio →
//!    `SYS_WRITE`);
//! 2. **allocate** — a heap `Vec` grown past several pages plus a `String` via
//!    `format!` (the `SYS_MAP_REGION`-backed, NX global allocator);
//! 3. **errno table** — `io::Error::from_raw_os_error(n)` decodes LogansOS
//!    errnos to the right `io::ErrorKind` and message (overlay `io/error`);
//! 4. **loud stubs** — every `sys/*` surface the overlay leaves to std's
//!    `unsupported` tier fails with `ErrorKind::Unsupported`, never a silent
//!    wrong answer (fs, net, process, thread, stdin, cwd, args);
//! 5. **exit** — `std::process::exit(0)` (overlay exit → `SYS_EXIT`).
//!
//! A failed check prints `FAIL` and exits with that check's number, so a
//! non-zero code in the kernel log localizes it. The panic path is the sibling
//! binary `std-real-panic`.
//!
//! **Entry:** the LogansOS ELF loader jumps to `e_entry` with zeroed registers,
//! `RSP = stack_top-8`, and **no argc/argv/auxv** (a bare `_start` contract),
//! so this is `#![no_main]` with a `_start` that drives `std` directly (stdout
//! initializes lazily; no `rt::lang_start`). **A1:** no executable memory; the
//! target is R+W+NX, `has_thread_local=false` (so no `PT_TLS` segment),
//! `singlethread=true`.

#![no_main]
// std built for an unofficial target marks itself `restricted_std`; opting in
// is the standard consumer acknowledgement of a custom-platform std.
#![feature(restricted_std)]

use std::io::{self, ErrorKind, Read, Write};

use neo_std_pal::SyscallHeap;

/// `std`'s `Vec`/`String`/`Box` route here (SYS_MAP_REGION-backed, NX heap).
#[global_allocator]
static HEAP: SyscallHeap = SyscallHeap::new();

const TAG: &str = "[ std-real-smoke ]";

fn check(code: i32, what: &str, ok: bool) {
    if ok {
        println!("{TAG} ok: {what}");
    } else {
        println!("{TAG} FAIL({code}): {what}");
        std::process::exit(code);
    }
}

fn unsupported<T>(r: io::Result<T>) -> bool {
    matches!(r, Err(ref e) if e.kind() == ErrorKind::Unsupported)
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    // 1. print
    println!("{TAG} std::println!: ok");
    eprintln!("{TAG} std::eprintln! (stderr -> the console fd): ok");
    check(
        10,
        "io::stdout().write_all + flush",
        io::stdout()
            .write_all(b"[ std-real-smoke ] raw stdout write\n")
            .is_ok()
            && io::stdout().flush().is_ok(),
    );

    // 2. allocate — 16 Ki u32 = 64 KiB, so the Vec regrows across many pages.
    let mut v: Vec<u32> = Vec::new();
    for i in 0..16 * 1024u32 {
        v.push(i.wrapping_mul(2654435761)); // Knuth hash: forces real heap use
    }
    let checksum = v.iter().fold(0u32, |a, &x| a ^ x);
    let s: String = format!("len={} checksum={:#010x}", v.len(), checksum);
    println!("{TAG} std heap Vec+String: {s}");
    check(
        20,
        "Vec grew to 16384 elements",
        v.len() == 16 * 1024 && v.capacity() >= v.len(),
    );
    let mut grown = String::new();
    for i in 0..512 {
        grown.push_str(&i.to_string());
    }
    check(21, "String grew via push_str", grown.len() == 1_426);
    drop(v);

    // 3. errno table — an independent expectation (the host KAT checks the
    //    whole table; this proves the overlay module is the one linked).
    for (code, errno, kind, n) in [
        (1, "EPERM", ErrorKind::PermissionDenied, 30),
        (2, "ENOENT", ErrorKind::NotFound, 31),
        (11, "EAGAIN", ErrorKind::WouldBlock, 32),
        (12, "ENOMEM", ErrorKind::OutOfMemory, 33),
        (22, "EINVAL", ErrorKind::InvalidInput, 34),
        (32, "EPIPE", ErrorKind::BrokenPipe, 35),
        (38, "ENOSYS", ErrorKind::Unsupported, 36),
        (110, "ETIMEDOUT", ErrorKind::TimedOut, 37),
    ] {
        let e = io::Error::from_raw_os_error(code);
        let msg = e.to_string();
        check(
            n,
            &format!("errno {code} -> {kind:?} \"{msg}\""),
            e.kind() == kind && msg.starts_with(errno) && msg.contains(&format!("errno {code}")),
        );
    }
    check(
        38,
        "unknown errno -> Uncategorized, numbered message",
        io::Error::from_raw_os_error(4242).to_string()
            == "unknown LogansOS errno 4242 (os error 4242)",
    );

    // 4. fs is fail-closed (C2.1b): this process holds no RootDirCap, so
    //    std refuses the open before any syscall.
    check(
        40,
        "fs::File::open without a RootDirCap -> PermissionDenied",
        matches!(std::fs::File::open("/etc/hosts"), Err(ref e) if e.kind() == ErrorKind::PermissionDenied),
    );
    // Loud stubs — every surface below is std's `unsupported` tier.
    check(
        41,
        "fs::read_dir -> Unsupported",
        unsupported(std::fs::read_dir("/")),
    );
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 80));
    check(
        42,
        "net::TcpStream::connect -> Unsupported",
        unsupported(std::net::TcpStream::connect(addr)),
    );
    check(
        43,
        "net::UdpSocket::bind -> Unsupported",
        unsupported(std::net::UdpSocket::bind(addr)),
    );
    check(
        44,
        "process::Command::spawn -> Unsupported",
        unsupported(std::process::Command::new("neo-init").spawn()),
    );
    check(
        45,
        "thread::Builder::spawn -> Unsupported",
        unsupported(std::thread::Builder::new().spawn(|| {})),
    );
    let mut buf = [0u8; 8];
    check(
        46,
        "io::stdin().read -> Unsupported (no fake EOF)",
        unsupported(io::stdin().read(&mut buf)),
    );
    check(
        47,
        "env::current_dir -> Unsupported",
        unsupported(std::env::current_dir()),
    );
    check(
        48,
        "env::args is empty (the loader passes no argv)",
        std::env::args_os().count() == 0,
    );
    check(
        49,
        "thread::available_parallelism -> Err",
        std::thread::available_parallelism().is_err(),
    );

    // 5. exit cleanly
    println!("{TAG} PASS: std print+alloc+errno+loud-stubs+exit");
    std::process::exit(0);
}
