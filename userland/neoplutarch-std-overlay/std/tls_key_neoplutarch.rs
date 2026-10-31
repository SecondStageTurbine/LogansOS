// SPDX-License-Identifier: MPL-2.0
//! OT-61 C1.3: `std::sys::thread_local::key::neoplutarch`, std's OS-key API on
//! the per-thread key layer (`sys::neoplutarch_rt::tls`, the file libneo
//! shares).
//!
//! std reaches this through its `os` thread-local route (the target has
//! `has-thread-local=false`, so there is no `PT_TLS`): `racy::LazyKey` creates
//! a key on first use, and `guard/key.rs` registers the key whose destructor
//! runs std's thread-local destructors and `thread_cleanup` at thread exit.
//! Each thread's values live in its own key block, found through FS.base, and
//! `std::thread`'s exit path runs the key destructors.

use crate::sys::neoplutarch_rt::tls;

pub type Key = usize;

#[inline]
pub fn create(dtor: Option<unsafe extern "C" fn(*mut u8)>) -> Key {
    // Keys start at 1, so std's "not yet created" sentinel 0 never comes back.
    match tls::create(dtor) {
        Some(key) => key,
        None => rtabort!("out of TLS keys"),
    }
}

#[inline]
pub unsafe fn set(key: Key, value: *mut u8) {
    if !tls::set(key, value) {
        rtabort!("TLS key set failed (key not live, or no memory for the key block)");
    }
}

#[inline]
pub unsafe fn get(key: Key) -> *mut u8 {
    tls::get(key)
}

#[inline]
pub unsafe fn destroy(key: Key) {
    tls::delete(key);
}
