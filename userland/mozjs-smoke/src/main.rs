// SPDX-License-Identifier: MPL-2.0
//! OT-61 C3c-1: SpiderMonkey (interpreter, one thread) on x86_64-unknown-neoplutarch,
//! over neo-crt (`docs/THINK-2026-11-08-ot61-c3c1-product-build.md` §4).
//!
//! One runtime, one context, one realm. Each check evaluates a script whose
//! value is an int32, and compares it with a hand-written answer:
//! 1. `1 + 1` (the interpreter);
//! 2. a closure that keeps a counter across calls;
//! 3. a regular expression (irregexp) with a capture group;
//! 4. `String.prototype.normalize` (the frozen normalizer_glue): NFC composes
//!    `A` + U+030A into U+00C5, NFD splits it back;
//! 5. `Date` from a fixed time value, read in UTC (no time zone lookup);
//! 6. a forced GC after allocating well past one 1 MiB chunk (trap T3), then
//!    the survivors are still intact.
//!
//! Exit 0 on a pass. A failed check prints `FAIL(n)` and exits `n`.

#![no_main]

use std::ptr;

use mozjs::jsapi::{GCReason, OnNewGlobalHookOption};
use mozjs::jsval::UndefinedValue;
use mozjs::rooted;
use mozjs::rust::wrappers2::{JS_GC, JS_NewGlobalObject};
use mozjs::rust::{
    evaluate_script, CompileOptionsWrapper, HandleObject, JSEngine, RealmOptions, Runtime,
    SIMPLE_GLOBAL_CLASS,
};
use neo_std_pal::SyscallHeap;

#[global_allocator]
static HEAP: SyscallHeap = SyscallHeap::new();

const TAG: &str = "[ mozjs-smoke ]";

/// (check, what, script, expected int32). Every script's value is an int32,
/// so a wrong type fails the same way a wrong value does.
const CHECKS: &[(i32, &str, &str, i32)] = &[
    (1, "1 + 1 == 2", "1 + 1", 2),
    (
        2,
        "a closure keeps its counter across three calls",
        "var counter = (function () { var n = 0; return function () { n += 1; return n; }; })();
         counter(); counter(); counter()",
        3,
    ),
    (
        3,
        "irregexp: /a(b+)c/ on 'xabbbcy' captures 3 b's at index 1",
        "var m = /a(b+)c/.exec('xabbbcy'); m.index * 100 + m[1].length",
        103,
    ),
    (
        4,
        "normalize: NFC('A\\u030A') is U+00C5, NFD splits it into 2 code units",
        "var c = 'A\\u030A'.normalize('NFC'); var d = '\\u00C5'.normalize('NFD');
         c.length * 1000000 + c.charCodeAt(0) * 100 + d.length * 10 + (d.charCodeAt(1) === 0x30A ? 1 : 0)",
        1_019_721,
    ),
    (
        5,
        "Date(1700000000000) in UTC is 2023-11-14 22:13:20",
        "var t = new Date(1700000000000);
         t.getUTCFullYear() * 10000 + (t.getUTCMonth() + 1) * 100 + t.getUTCDate()",
        20_231_114,
    ),
    (
        6,
        "UTC time of day is 22:13:20",
        "var t = new Date(1700000000000); t.getUTCHours() * 10000 + t.getUTCMinutes() * 100 + t.getUTCSeconds()",
        221_320,
    ),
];

/// Allocates about 4 MiB of objects (each with a 64-byte string), more than
/// one GC chunk, and keeps every 16th one alive.
const ALLOCATE: &str = "var keep = [];
    for (var i = 0; i < 40000; i++) {
        var o = { i: i, s: 'x'.repeat(64) + i };
        if (i % 16 === 0) keep.push(o);
    }
    keep.length";
/// After the GC: every survivor still has its index and its string.
const SURVIVORS: &str = "var ok = 0;
    for (var j = 0; j < keep.length; j++) {
        var o = keep[j];
        if (o.i === j * 16 && o.s === 'x'.repeat(64) + (j * 16)) ok++;
    }
    ok";

/// Evaluates `script` in `global`'s realm and returns its int32 value, or
/// None for an exception or a value that isn't an int32.
fn eval(rt: &mut Runtime, global: HandleObject, script: &str) -> Option<i32> {
    rooted!(&in(rt.cx()) let mut rval = UndefinedValue());
    let opts = CompileOptionsWrapper::new(rt.cx_no_gc(), c"smoke.js".to_owned(), 1);
    evaluate_script(rt.cx(), global, script, rval.handle_mut(), opts).ok()?;
    let v = rval.get();
    v.is_int32().then(|| v.to_int32())
}

fn check(n: i32, what: &str, ok: bool) {
    if ok {
        println!("{TAG} ok({n}) {what}");
    } else {
        println!("{TAG} FAIL({n}) {what}");
        std::process::exit(n);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    // SpiderMonkey is compiled with the stack protector: seed the canary
    // before any C++ runs, then run the static constructors.
    neo_crt::seed_stack_canary();
    neo_crt::init();
    println!("{TAG} start: SpiderMonkey 153.6 (mozjs 0.26.14), interpreter, one thread");

    let engine = JSEngine::init().unwrap_or_else(|e| {
        println!("{TAG} FAIL(10) JSEngine::init: {e:?}");
        std::process::exit(10)
    });
    let mut rt = Runtime::new(engine.handle());
    let options = RealmOptions::default();
    rooted!(&in(rt.cx()) let global = unsafe {
        JS_NewGlobalObject(
            rt.cx(),
            &SIMPLE_GLOBAL_CLASS,
            ptr::null_mut(),
            OnNewGlobalHookOption::FireOnNewGlobalHook,
            &*options,
        )
    });
    check(11, "a runtime, a context and a global in one realm", !global.get().is_null());

    for &(n, what, script, want) in CHECKS {
        let got = eval(&mut rt, global.handle(), script);
        println!("{TAG}   check {n}: got {got:?}, want {want}");
        check(n, what, got == Some(want));
    }

    let kept = eval(&mut rt, global.handle(), ALLOCATE);
    println!("{TAG}   check 7: allocated 40000 objects, kept {kept:?}");
    check(7, "about 4 MiB allocated (past one chunk), 2500 kept", kept == Some(2500));
    unsafe { JS_GC(rt.cx(), GCReason::API) };
    let ok = eval(&mut rt, global.handle(), SURVIVORS);
    println!("{TAG}   check 8: after JS_GC, {ok:?} of 2500 survivors intact");
    check(8, "after a forced GC every survivor is intact", ok == Some(2500));

    println!("{TAG} PASS");
    std::process::exit(0)
}
