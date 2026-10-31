// SPDX-License-Identifier: MPL-2.0
//! OT-61 rung 6: `browser_io_smoke`'s peer. It waits (parked) for the
//! smoke's `G`, then sends `W`, which parks it until the smoke's `try_recv`
//! takes it. Holding the send until the go makes the smoke's first
//! `try_recv` a deterministic `WouldBlock`.
//!
//! Exit 0 once `W` is taken; a failure prints `FAIL(n)` and exits `n`.

#![no_main]
#![feature(neoplutarch_ext)]

use std::os::neoplutarch::{HandleKind, channel, handles};

use neo_std_pal::SyscallHeap;

#[global_allocator]
static HEAP: SyscallHeap = SyscallHeap::new();

const TAG: &str = "[ browser-io-peer ]";

fn fail(n: i32, what: &str) -> ! {
    println!("{TAG} FAIL({n}) {what}");
    std::process::exit(n);
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let chans: Vec<_> = handles()
        .filter(|(_, info)| info.kind == HandleKind::Channel)
        .map(|(h, _)| h)
        .collect();
    if chans.len() != 1 {
        fail(1, "the launcher handed exactly one channel");
    }
    let mut go = [0u8; 4];
    match channel::recv(chans[0], &mut go) {
        Ok(1) if go[0] == b'G' => {}
        r => fail(2, &format!("receive the go: {r:?}")),
    }
    match channel::send(chans[0], b"W") {
        Ok(1) => {}
        r => fail(3, &format!("send W: {r:?}")),
    }
    println!("{TAG} PASS");
    std::process::exit(0)
}
