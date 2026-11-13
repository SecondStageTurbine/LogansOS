// SPDX-License-Identifier: MPL-2.0
//! OT-61 C3c-2b: `mozjs-smoke-oom`. SpiderMonkey starts its helper pool, puts
//! it to work, then the process dies with the helpers alive
//! (`docs/THINK-2026-11-12-ot61-c3c2-helper-threads.md` §4.3). The kernel
//! must see only this process go. The checks are in `smoke.rs`.

#![no_main]

#[path = "../smoke.rs"]
mod smoke;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    smoke::run(smoke::Mode::Oom)
}
