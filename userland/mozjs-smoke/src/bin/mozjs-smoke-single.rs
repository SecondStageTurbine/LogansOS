// SPDX-License-Identifier: MPL-2.0
//! OT-61 C3c-2: `mozjs-smoke-single`, the helper-thread kill-switch.
//! SpiderMonkey runs with its pool switched off before the engine starts
//! (`neo_js_disable_extra_threads`, patch 0006), and must give C3c-1's
//! results with no thread made. The checks are in `smoke.rs`.

#![no_main]

#[path = "../smoke.rs"]
mod smoke;

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    smoke::run(smoke::Mode::Single)
}
