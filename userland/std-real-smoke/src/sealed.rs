// SPDX-License-Identifier: MPL-2.0
//! OT-61 C2.1b — real `std` in a sealed process (`std_sealed` scenario).
//!
//! The kernel spawns this binary three times, handing it nothing, a read-only
//! RootDirCap, or a full one. It picks its role from what it holds
//! (`std::os::neoplutarch::handles`). std's file access has no ambient
//! namespace: it goes only through a RootDirCap the process was given.
//!
//! - **No handles:** `File::open` is `PermissionDenied` (refusal 1); the
//!   environment is empty and `process::id` works. Exit 51.
//! - **A read-only RootDirCap:** a read open works; a write open is
//!   `PermissionDenied` (refusal 2); after the program drops its only cap, a
//!   new open is `PermissionDenied` (refusal 3) while the file it already had
//!   open still reads. Exit 52.
//! - **A full RootDirCap:** write, sync, reopen and read back; a raw-handle
//!   round trip. Exit 0.
//!
//! A failed check prints `FAIL(n)` and exits with `n`.

#![no_main]
#![feature(restricted_std)]
#![feature(neoplutarch_ext)]

use std::fs::{File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::os::neoplutarch::io::{FromRawHandle, IntoRawHandle};
use std::os::neoplutarch::{HandleKind, handles};

use neo_std_pal::SyscallHeap;

#[global_allocator]
static HEAP: SyscallHeap = SyscallHeap::new();

const TAG: &str = "[ std-sealed ]";
const SCRATCH: &str = "/scratch";
const FILE_READ: u32 = 1 << 0;
const FILE_WRITE: u32 = 1 << 1;

const NO_CAP_EXIT: i32 = 51;
const READ_ONLY_EXIT: i32 = 52;

fn check(code: i32, what: &str, ok: bool) {
    if ok {
        println!("{TAG} ok: {what}");
    } else {
        println!("{TAG} FAIL({code}): {what}");
        std::process::exit(code);
    }
}

fn denied<T>(r: std::io::Result<T>) -> bool {
    matches!(r, Err(e) if e.kind() == ErrorKind::PermissionDenied)
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let held: Vec<_> = handles().collect();
    let pid = std::process::id();
    check(1, "process::id is a pid", pid > 0 && std::process::id() == pid);
    check(
        2,
        "the environment is empty",
        std::env::vars().next().is_none() && std::env::var("HOME").is_err(),
    );
    println!("{TAG} pid={pid} handles={}", held.len());

    // Role: nothing handed in.
    if held.is_empty() {
        check(3, "no cap: File::open is PermissionDenied", denied(File::open(SCRATCH)));
        check(
            4,
            "no cap: a write open is PermissionDenied",
            denied(OpenOptions::new().write(true).open(SCRATCH)),
        );
        println!("{TAG} PASS (no-cap role): std has no ambient file namespace");
        std::process::exit(NO_CAP_EXIT);
    }

    check(5, "exactly one handle: the RootDirCap", held.len() == 1);
    let (cap, info) = held[0];
    check(
        6,
        "it is a RootDirCap with READ",
        info.kind == HandleKind::File && info.root_dir && info.file_rights & FILE_READ != 0,
    );

    // Role: a read-only RootDirCap.
    if info.file_rights & FILE_WRITE == 0 {
        let mut f = match File::open(SCRATCH) {
            Ok(f) => f,
            Err(e) => {
                println!("{TAG} FAIL(7): read open through the cap: {e:?}");
                std::process::exit(7);
            }
        };
        let mut buf = [0u8; 8];
        check(8, "read through the cap", f.read(&mut buf).is_ok());
        check(
            9,
            "read-only cap: a write open is PermissionDenied",
            denied(OpenOptions::new().write(true).open(SCRATCH)),
        );
        // Drop the only cap: nothing left to open through.
        // SAFETY: `cap` is this process's own RootDirCap, owned by nothing else.
        drop(unsafe { File::from_raw_handle(cap) });
        check(10, "the cap is gone from the table", handles().all(|(h, _)| h != cap));
        check(
            11,
            "after dropping the cap: File::open is PermissionDenied",
            denied(File::open(SCRATCH)),
        );
        check(
            12,
            "a file opened before still reads (a close is not a revoke)",
            f.read(&mut buf).is_ok(),
        );
        println!("{TAG} PASS (read-only role): reads only, and nothing after the cap is dropped");
        std::process::exit(READ_ONLY_EXIT);
    }

    // Role: a full RootDirCap.
    let msg = b"sealed std";
    let mut w = match OpenOptions::new().write(true).open(SCRATCH) {
        Ok(f) => f,
        Err(e) => {
            println!("{TAG} FAIL(13): write open through the cap: {e:?}");
            std::process::exit(13);
        }
    };
    check(14, "write", w.write_all(msg).is_ok());
    check(15, "sync_all", w.sync_all().is_ok());
    drop(w);
    let r = File::open(SCRATCH).expect("read open");
    // A raw-handle round trip keeps the same open file.
    let raw = r.into_raw_handle();
    // SAFETY: `raw` came from into_raw_handle just above; nothing else owns it.
    let mut r = unsafe { File::from_raw_handle(raw) };
    let mut back = [0u8; 10];
    check(16, "read back what was written", r.read_exact(&mut back).is_ok() && &back == msg);
    println!("{TAG} PASS (full role): write, sync and read back through the cap");
    std::process::exit(0);
}
