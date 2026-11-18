// SPDX-License-Identifier: MPL-2.0
//! OT-61 C3c-1/C3c-2: `mozjs-smoke`, SpiderMonkey with its helper thread pool
//! on. The checks are in `smoke.rs`, shared with `mozjs-smoke-single`.

#![no_main]

mod smoke;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    smoke::run(smoke::Mode::Pool)
}
