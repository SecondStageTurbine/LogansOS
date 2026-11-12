// SPDX-License-Identifier: MPL-2.0
//! OT-61 C3c-1 and C3c-2: SpiderMonkey on x86_64-unknown-neoplutarch, over
//! neo-crt (`docs/THINK-2026-11-08-ot61-c3c1-product-build.md` §4,
//! `docs/THINK-2026-11-12-ot61-c3c2-helper-threads.md` §4).
//!
//! One runtime, one context, one realm. Both binaries run every check:
//! - `mozjs-smoke` with SpiderMonkey's helper thread pool (C3c-2);
//! - `mozjs-smoke-single` with the pool switched off before the engine starts
//!   (`neo_js_disable_extra_threads`, patch 0006): the kill-switch, which must
//!   give C3c-1's results with no thread made.
//!
//! C3c-1's checks (each script's value is an int32, compared with a
//! hand-written answer):
//! - 12: a 1 MiB block at 1 MiB alignment from neo-crt's `posix_memalign`
//!   (a GC chunk's shape), before the engine starts;
//! - 11: a runtime, a context and a global;
//! - 1-6: `1 + 1`, a closure, irregexp, `normalize`, `Date` in UTC;
//! - 7, 8: about 4 MiB allocated, a forced GC, the survivors intact.
//!
//! C3c-2's checks, counted through neo-crt's thread record:
//! - 20: the helper threads started (pool: `JSGC_HELPER_THREAD_COUNT`
//!   threads, at least 2; single: none);
//! - 26: the control for check 21: with parallel marking and compaction off
//!   and a small allocation, no helper thread is woken;
//! - 24, 25: a shrinking GC packs a fragmented heap (fewer heap bytes than
//!   after a normal GC of the same heap, no unused chunk kept) and every
//!   survivor keeps its value after its object moved;
//! - 21: helper threads were woken for work before shutdown (single: never);
//! - 22: after `JS_ShutDown`, every helper thread exited and was joined.
//!
//! Exit 0 on a pass. A failed check prints `FAIL(n)` and exits `n`.

use std::ptr;
use std::thread;
use std::time::Duration;

use mozjs::jsapi::{GCOptions, GCReason, JSGCParamKey, OnNewGlobalHookOption};
use mozjs::jsval::UndefinedValue;
use mozjs::rooted;
use mozjs::rust::wrappers2::{
    JS_GC, JS_GetGCParameter, JS_NewGlobalObject, JS_SetGCParameter, NonIncrementalGC,
    PrepareForFullGC,
};
use mozjs::rust::{
    CompileOptionsWrapper, HandleObject, JSEngine, RealmOptions, Runtime, SIMPLE_GLOBAL_CLASS,
    evaluate_script,
};
use neo_std_pal::SyscallHeap;

#[global_allocator]
static HEAP: SyscallHeap = SyscallHeap::new();

unsafe extern "C" {
    fn posix_memalign(out: *mut *mut core::ffi::c_void, align: usize, size: usize) -> i32;
    fn free(p: *mut core::ffi::c_void);
    /// Patch 0006: `js::DisableExtraThreads()`, for our target only.
    fn neo_js_disable_extra_threads();
}

/// Which binary is running. Each binary constructs only its own variant.
#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// SpiderMonkey's helper thread pool on (C3c-2).
    Pool,
    /// The pool switched off before the engine starts (the kill-switch).
    Single,
}

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

/// Check 26's work: a hundred small objects, too little to start a GC.
const SMALL: &str = "var small = [];
    for (var i = 0; i < 100; i++) small.push({ i: i });
    small.length";

/// Fragments the heap: about 8 MiB of objects and strings, of which every
/// 16th survives. Nearly every arena keeps a few survivors, so a normal GC
/// frees few arenas; only compaction can pack them.
const FRAGMENT: &str = "var kept = []; var junk = [];
    for (var i = 0; i < 80000; i++) {
        var o = { i: i, s: 'y'.repeat(64) + i };
        if (i % 16 === 0) kept.push(o); else junk.push(o);
    }
    junk = null;
    kept.length";
/// After the shrinking GC: every survivor (moved) still has its values.
const MOVED: &str = "var ok = 0;
    for (var j = 0; j < kept.length; j++) {
        var o = kept[j];
        if (o.i === j * 16 && o.s === 'y'.repeat(64) + (j * 16)) ok++;
    }
    ok";

struct Out {
    tag: &'static str,
}

impl Out {
    fn check(&self, n: i32, what: &str, ok: bool) {
        if ok {
            println!("{} ok({n}) {what}", self.tag);
        } else {
            println!("{} FAIL({n}) {what}", self.tag);
            std::process::exit(n);
        }
    }
}

/// Evaluates `script` in `global`'s realm and returns its int32 value, or
/// None for an exception or a value that isn't an int32.
fn eval(rt: &mut Runtime, global: HandleObject, script: &str) -> Option<i32> {
    rooted!(&in(rt.cx()) let mut rval = UndefinedValue());
    let opts = CompileOptionsWrapper::new(rt.cx_no_gc(), c"smoke.js".to_owned(), 1);
    evaluate_script(rt.cx(), global, script, rval.handle_mut(), opts).ok()?;
    let v = rval.get();
    v.is_int32().then(|| v.to_int32())
}

fn gc_param(rt: &Runtime, key: JSGCParamKey) -> u32 {
    unsafe { JS_GetGCParameter(rt.cx_no_gc(), key) }
}

/// The thread record once it has stopped moving: no change across five reads
/// 10 ms apart. A helper that is still starting, or still settling after its
/// last task, would otherwise land its wake inside the next measurement.
fn settled() -> neo_crt::ThreadCounts {
    let mut last = neo_crt::thread_counts();
    let mut same = 0;
    while same < 5 {
        thread::sleep(Duration::from_millis(10));
        let now = neo_crt::thread_counts();
        if now == last {
            same += 1;
        } else {
            same = 0;
            last = now;
        }
    }
    last
}

/// A full, non-incremental GC of the given kind.
fn full_gc(rt: &mut Runtime, options: GCOptions) {
    unsafe {
        PrepareForFullGC(rt.cx_no_gc());
        NonIncrementalGC(rt.cx(), options, GCReason::API);
    }
}

pub fn run(mode: Mode) -> ! {
    let out = Out {
        tag: match mode {
            Mode::Pool => "[ mozjs-smoke ]",
            Mode::Single => "[ mozjs-smoke-single ]",
        },
    };
    let tag = out.tag;

    // SpiderMonkey is compiled with the stack protector: seed the canary
    // before any C++ runs, then run the static constructors.
    neo_crt::seed_stack_canary();
    neo_crt::init();
    let threads_note = match mode {
        Mode::Pool => "helper threads on",
        Mode::Single => "helper threads off",
    };
    println!("{tag} start: SpiderMonkey 153.6 (mozjs 0.26.14), interpreter, {threads_note}");

    const MIB: usize = 1 << 20;
    let mut block = ptr::null_mut();
    let err = unsafe { posix_memalign(&mut block, MIB, MIB) };
    println!("{tag}   check 12: posix_memalign(1 MiB, 1 MiB) = {err}, {block:p}");
    out.check(
        12,
        "a 1 MiB-aligned 1 MiB block (a GC chunk's shape) from posix_memalign",
        err == 0 && !block.is_null() && (block as usize) % MIB == 0,
    );
    unsafe { free(block) };

    if mode == Mode::Single {
        // Before JS_Init and the first runtime: the pool starts in
        // JSRuntime::init only while extra threads are allowed.
        unsafe { neo_js_disable_extra_threads() };
    }

    let engine = JSEngine::init().unwrap_or_else(|e| {
        println!("{tag} FAIL(10) JSEngine::init: {e:?}");
        std::process::exit(10)
    });
    let pool;
    let before_shutdown;
    {
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
        out.check(
            11,
            "a runtime, a context and a global in one realm",
            !global.get().is_null(),
        );

        // Check 20: the pool's size, from SpiderMonkey and from neo-crt.
        let started = settled();
        match mode {
            Mode::Pool => {
                let helpers = gc_param(&rt, JSGCParamKey::JSGC_HELPER_THREAD_COUNT);
                pool = started.created;
                println!(
                    "{tag}   check 20: JSGC_HELPER_THREAD_COUNT {helpers}, pthread_create {}, available_parallelism {:?}",
                    started.created,
                    thread::available_parallelism().map(|n| n.get()),
                );
                out.check(
                    20,
                    "the helper thread pool started (JSGC_HELPER_THREAD_COUNT threads, at least 2)",
                    started.created >= 2 && u64::from(helpers) == started.created,
                );
            }
            Mode::Single => {
                pool = 0;
                println!("{tag}   check 20: pthread_create {}", started.created);
                out.check(
                    20,
                    "no helper thread started (neo_js_disable_extra_threads)",
                    started.created == 0,
                );
            }
        }

        // Check 26, the control for check 21: no work for a helper, so no
        // helper is woken. If this fails, the wake count is measuring
        // something other than tasks, and check 21 can't be trusted.
        let marking = gc_param(&rt, JSGCParamKey::JSGC_PARALLEL_MARKING_ENABLED);
        let compacting = gc_param(&rt, JSGCParamKey::JSGC_COMPACTING_ENABLED);
        unsafe {
            JS_SetGCParameter(rt.cx(), JSGCParamKey::JSGC_PARALLEL_MARKING_ENABLED, 0);
            JS_SetGCParameter(rt.cx(), JSGCParamKey::JSGC_COMPACTING_ENABLED, 0);
        }
        let w0 = settled().wakes;
        let small = eval(&mut rt, global.handle(), SMALL);
        let w1 = settled().wakes;
        println!(
            "{tag}   check 26: parallel marking {marking} -> 0, compacting {compacting} -> 0; {small:?} objects; helper wakes {w0} -> {w1}"
        );
        out.check(
            26,
            "control: no helper wake while there is no work (parallel marking and compaction off, a small allocation)",
            small == Some(100) && w1 == w0,
        );
        unsafe {
            JS_SetGCParameter(
                rt.cx(),
                JSGCParamKey::JSGC_PARALLEL_MARKING_ENABLED,
                marking,
            );
            JS_SetGCParameter(rt.cx(), JSGCParamKey::JSGC_COMPACTING_ENABLED, compacting);
        }

        for &(n, what, script, want) in CHECKS {
            let got = eval(&mut rt, global.handle(), script);
            println!("{tag}   check {n}: got {got:?}, want {want}");
            out.check(n, what, got == Some(want));
        }

        let kept = eval(&mut rt, global.handle(), ALLOCATE);
        println!("{tag}   check 7: allocated 40000 objects, kept {kept:?}");
        out.check(
            7,
            "about 4 MiB allocated (past one chunk), 2500 kept",
            kept == Some(2500),
        );
        unsafe { JS_GC(rt.cx(), GCReason::API) };
        let ok = eval(&mut rt, global.handle(), SURVIVORS);
        println!("{tag}   check 8: after JS_GC, {ok:?} of 2500 survivors intact");
        out.check(
            8,
            "after a forced GC every survivor is intact",
            ok == Some(2500),
        );

        // Checks 24 and 25: a normal GC of a fragmented heap, then a
        // shrinking one. Compaction must be on (it is SpiderMonkey's default).
        out.check(
            27,
            "compacting GC is enabled (JSGC_COMPACTING_ENABLED)",
            gc_param(&rt, JSGCParamKey::JSGC_COMPACTING_ENABLED) == 1,
        );
        let kept = eval(&mut rt, global.handle(), FRAGMENT);
        full_gc(&mut rt, GCOptions::Normal);
        let (bytes_a, total_a, unused_a) = (
            gc_param(&rt, JSGCParamKey::JSGC_BYTES),
            gc_param(&rt, JSGCParamKey::JSGC_TOTAL_CHUNKS),
            gc_param(&rt, JSGCParamKey::JSGC_UNUSED_CHUNKS),
        );
        full_gc(&mut rt, GCOptions::Shrink);
        let (bytes_b, total_b, unused_b) = (
            gc_param(&rt, JSGCParamKey::JSGC_BYTES),
            gc_param(&rt, JSGCParamKey::JSGC_TOTAL_CHUNKS),
            gc_param(&rt, JSGCParamKey::JSGC_UNUSED_CHUNKS),
        );
        println!(
            "{tag}   check 24: kept {kept:?}; normal GC: {bytes_a} bytes, {total_a} chunks ({unused_a} unused); shrinking GC: {bytes_b} bytes, {total_b} chunks ({unused_b} unused)"
        );
        out.check(
            24,
            "a shrinking GC packed the fragmented heap (fewer heap bytes than a normal GC left, no unused chunk kept)",
            kept == Some(5000) && bytes_b < bytes_a && unused_b == 0,
        );
        let moved = eval(&mut rt, global.handle(), MOVED);
        println!("{tag}   check 25: after the shrinking GC, {moved:?} of 5000 survivors intact");
        out.check(
            25,
            "after the shrinking GC every survivor is intact",
            moved == Some(5000),
        );

        // Check 21: helper threads were handed work (the compacting GC's
        // parallel tasks, background sweeping and freeing).
        before_shutdown = settled();
        println!(
            "{tag}   check 21: helper wakes {}, timeouts {} before shutdown",
            before_shutdown.wakes, before_shutdown.timeouts
        );
        match mode {
            Mode::Pool => out.check(
                21,
                "helper threads were woken for work before shutdown",
                before_shutdown.wakes >= 1,
            ),
            Mode::Single => out.check(
                21,
                "no helper thread was ever woken",
                before_shutdown.wakes == 0,
            ),
        }
        // The roots, then the runtime, go here; the engine stays.
    }

    // Check 22: JS_ShutDown (the engine's drop) stops and joins the pool.
    drop(engine);
    let after = neo_crt::thread_counts();
    println!(
        "{tag}   check 22: created {}, exited {}, joined {} after JS_ShutDown (pool {pool}, {} before)",
        after.created, after.exited, after.joined, before_shutdown.created
    );
    match mode {
        Mode::Pool => out.check(
            22,
            "JS_ShutDown stopped and joined every helper thread",
            after.created == pool && after.exited == pool && after.joined == pool,
        ),
        Mode::Single => out.check(
            22,
            "no thread to join: none was ever created",
            after.created == 0 && after.exited == 0 && after.joined == 0,
        ),
    }

    println!("{tag} PASS");
    std::process::exit(0)
}
