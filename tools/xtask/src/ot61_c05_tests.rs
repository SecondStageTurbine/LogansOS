// SPDX-License-Identifier: MPL-2.0
//! OT-61 C0.5 host tests (run by push-CI host checks: `cargo test --workspace`).
//!
//! 1. **A4 manifest gate teeth.** The gate's checker (`userland/neo-std-pal/a4_gate.rs`,
//!    the same file `neo-std-pal/build.rs` runs) passes on the real tree and FAILS on
//!    each committed negative fixture: one per rule (G1–G5), plus a replay of the
//!    pre-C0.5-completion bypass (the exit arm's syscall living in the overlay
//!    *script*, which the old gate never scanned).
//! 2. **The io-errno table KAT.** `io_error_neoplutarch.rs`'s `ERRNO_TABLE` is
//!    exhaustive over `kernel/src/errno.rs` (same names, same numbers) and agrees
//!    with the independent hand-written expectation below. Nothing maps to
//!    `Interrupted`.
//! 3. **Overlay consistency.** Every overlay module is installed by
//!    `apply-overlay.py` (there is no unscanned or orphaned module), and the abort
//!    exit code agrees between the overlay and the kernel scenario.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::a4_gate::{self, Source, check};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else if p
            .extension()
            .is_some_and(|x| x == "rs" || x == "sh" || x == "py")
        {
            out.push(p);
        }
    }
}

/// The real scanned surface, exactly as `neo-std-pal/build.rs` collects it.
fn real_sources() -> Vec<Source> {
    let mut paths = Vec::new();
    walk(&root().join("userland/neo-std-pal/src"), &mut paths);
    walk(&root().join("userland/neoplutarch-std-overlay"), &mut paths);
    paths.sort();
    paths
        .into_iter()
        .map(|p| Source {
            path: p.display().to_string(),
            text: fs::read_to_string(&p).unwrap(),
        })
        .collect()
}

fn gate(sources: &[Source]) -> Result<a4_gate::Summary, Vec<String>> {
    check(
        sources,
        &read("tools/ot61-pal-manifest.toml"),
        &read("kernel/src/arch/x86_64/syscall.rs"),
    )
}

/// The real tree plus one extra file.
fn with_extra(path: &str, text: &str) -> Vec<Source> {
    let mut s = real_sources();
    s.push(Source {
        path: path.to_string(),
        text: text.to_string(),
    });
    s
}

/// The real tree with `needle` in the overlay's syscall seam replaced.
fn with_rt_edit(needle: &str, replacement: &str) -> Vec<Source> {
    let mut s = real_sources();
    let rt = s
        .iter_mut()
        .find(|f| f.path.ends_with("neoplutarch_rt.rs"))
        .expect("overlay neoplutarch_rt.rs is scanned");
    assert_eq!(
        rt.text.matches(needle).count(),
        1,
        "fixture needle {needle:?}"
    );
    rt.text = rt.text.replace(needle, replacement);
    s
}

#[track_caller]
fn assert_refused(sources: &[Source], rule: &str) {
    match gate(sources) {
        Ok(s) => panic!("A4 gate PASSED a {rule} violation: {s:?}"),
        Err(errs) => assert!(
            errs.iter().any(|e| e.starts_with(rule)),
            "expected a {rule} finding, got {errs:#?}"
        ),
    }
}

// ── 1. A4 gate ───────────────────────────────────────────────────────────────

#[test]
fn a4_gate_passes_the_real_pal_and_overlay() {
    let s =
        gate(&real_sources()).unwrap_or_else(|e| panic!("A4 gate refused the real tree: {e:#?}"));
    let mut want: Vec<String> = C13_SYSCALLS
        .iter()
        .chain(C20_SYSCALLS.iter())
        .chain(C21B_SYSCALLS.iter())
        .map(|s| s.to_string())
        .collect();
    want.extend(["SYS_EXIT", "SYS_MAP_REGION", "SYS_WRITE"].map(String::from));
    want.sort();
    assert_eq!(s.referenced, want);
    // One trampoline in the PAL crate, one in the overlay's neoplutarch_rt.
    assert_eq!(s.trampolines, 2);
    // The one `fn tcb` read, in the shared TLS key layer.
    assert_eq!(s.tcb_reads, 1);
}

/// The ten C1 calls C1.3 adds to `[gate].allowed_syscalls`.
const C13_SYSCALLS: [&str; 10] = [
    "SYS_CLOCK_GETTIME",
    "SYS_FUTEX_WAIT",
    "SYS_FUTEX_WAKE",
    "SYS_THREAD_CREATE",
    "SYS_THREAD_EXIT",
    "SYS_THREAD_JOIN",
    "SYS_THREAD_DETACH",
    "SYS_THREAD_GET_INFO",
    "SYS_THREAD_SET_TLS",
    "SYS_SCHED_YIELD",
];

/// The C2.0 addition to `[gate].allowed_syscalls`.
const C20_SYSCALLS: [&str; 1] = ["SYS_GETRANDOM"];

/// The six calls C2.1b adds (std::fs, process::id, std::os::neoplutarch).
const C21B_SYSCALLS: [&str; 6] = [
    "SYS_OPEN",
    "SYS_READ",
    "SYS_CLOSE",
    "SYS_FSYNC",
    "SYS_PROCESS_SELF",
    "SYS_HANDLE_INFO",
];

/// The real tree with `needle` in the shared TLS key layer replaced.
fn with_tls_edit(needle: &str, replacement: &str) -> Vec<Source> {
    let mut s = real_sources();
    let tls = s
        .iter_mut()
        .find(|f| f.path.replace('\\', "/").ends_with(a4_gate::TCB_FILE))
        .expect("the TLS key layer is scanned");
    assert_eq!(
        tls.text.matches(needle).count(),
        1,
        "fixture needle {needle:?}"
    );
    tls.text = tls.text.replace(needle, replacement);
    s
}

#[test]
fn a4_g1_refuses_each_c13_syscall_once_unlisted() {
    let manifest = read("tools/ot61-pal-manifest.toml");
    for name in C13_SYSCALLS
        .iter()
        .chain(C20_SYSCALLS.iter())
        .chain(C21B_SYSCALLS.iter())
    {
        let name = *name;
        let quoted = format!("\"{name}\"");
        assert!(
            manifest.contains(&format!("{quoted}, ")) || manifest.contains(&format!(", {quoted}"))
        );
        let without = manifest
            .replace(&format!("{quoted}, "), "")
            .replace(&format!(", {quoted}"), "");
        match check(
            &real_sources(),
            &without,
            &read("kernel/src/arch/x86_64/syscall.rs"),
        ) {
            Ok(_) => panic!("A4 gate passed with {name} removed from the manifest"),
            Err(errs) => assert!(
                errs.iter()
                    .any(|e| e.starts_with("G1") && e.contains(&format!("`{name}`"))),
                "{name}: expected a G1 finding, got {errs:#?}"
            ),
        }
        // And each has its provenance line.
        assert!(
            manifest.contains(&format!("\n{name} = \"")),
            "{name} has no [gate.provenance] line"
        );
    }
}

#[test]
fn a4_g2_refuses_a_c13_syscall_bound_to_a_neighbouring_number() {
    // SYS_FUTEX_WAIT bound to SYS_FUTEX_WAKE's number would turn every wait
    // into a wake.
    assert_refused(
        &with_rt_edit(
            "const SYS_FUTEX_WAIT: u64 = 0x44;",
            "const SYS_FUTEX_WAIT: u64 = 0x45;",
        ),
        "G2",
    );
}

#[test]
fn a4_g2_refuses_a_c21b_syscall_bound_to_a_neighbouring_number() {
    // SYS_HANDLE_INFO bound to SYS_PROCESS_SELF's number, and SYS_READ bound
    // to SYS_CLOSE's, would each silently call the wrong kernel entry.
    assert_refused(
        &with_rt_edit(
            "const SYS_HANDLE_INFO: u64 = 0x51;",
            "const SYS_HANDLE_INFO: u64 = 0x50;",
        ),
        "G2",
    );
    assert_refused(
        &with_rt_edit("const SYS_READ: u64 = 13;", "const SYS_READ: u64 = 14;"),
        "G2",
    );
}

#[test]
fn a4_g2_refuses_sys_getrandom_bound_to_another_number() {
    // 0x191 is not the kernel's SYS_GETRANDOM (0x190).
    assert_refused(
        &with_rt_edit(
            "const SYS_GETRANDOM: u64 = 0x190;",
            "const SYS_GETRANDOM: u64 = 0x191;",
        ),
        "G2",
    );
}

#[test]
fn a4_g3_refuses_a_second_tcb_read() {
    let second = "fn tcb() -> u64 {\n    let v: u64;\n    unsafe { core::arch::asm!(\"mov {}, qword ptr fs:[0]\", out(reg) v) };\n    v\n}\n";
    assert_refused(&with_extra("overlay/std/t.rs", second), "G3");
    // Not even in the TCB file itself.
    assert_refused(
        &with_tls_edit(
            "/// The calling thread's control block.\n",
            &format!("{second}/// The calling thread's control block.\n"),
        ),
        "G3",
    );
}

#[test]
fn a4_g3_refuses_any_other_tcb_template() {
    for other in [
        "\"mov {}, qword ptr fs:[8]\"",
        "\"mov {}, qword ptr gs:[0]\"",
        "\"syscall\"",
        "\"mov {}, qword ptr fs:[0]; syscall\"",
    ] {
        assert_refused(
            &with_tls_edit("\"mov {}, qword ptr fs:[0]\",", &format!("{other},")),
            "G3",
        );
    }
}

#[test]
fn a4_g3_refuses_a_second_template_string() {
    // `asm!` concatenates template strings: the first one alone must not pass.
    assert_refused(
        &with_tls_edit(
            "\"mov {}, qword ptr fs:[0]\",",
            "\"mov {}, qword ptr fs:[0]\", \"syscall\",",
        ),
        "G3",
    );
    assert_refused(
        &with_tls_edit(
            "\"mov {}, qword ptr fs:[0]\",",
            "\"mov {}, qword ptr fs:[0]\", r\"syscall\",",
        ),
        "G3",
    );
    assert_refused(
        &with_rt_edit("\"syscall\",", "\"syscall\", \"int 0x80\","),
        "G3",
    );
}

#[test]
fn a4_g3_refuses_asm_elsewhere_in_the_tcb_file() {
    let extra = "fn other() -> u64 {\n    let v: u64;\n    unsafe { core::arch::asm!(\"mov {}, qword ptr fs:[0]\", out(reg) v) };\n    v\n}\n";
    assert_refused(
        &with_tls_edit(
            "/// The calling thread's control block.\n",
            &format!("{extra}/// The calling thread's control block.\n"),
        ),
        "G3",
    );
}

#[test]
fn a4_g3_refuses_a_second_asm_inside_fn_tcb() {
    assert_refused(
        &with_tls_edit(
            "    block as *mut ThreadControlBlock\n}",
            "    unsafe { core::arch::asm!(\"mov {}, qword ptr fs:[0]\", out(reg) _) };\n    block as *mut ThreadControlBlock\n}",
        ),
        "G3",
    );
}

#[test]
fn a4_g3_requires_the_tcb_file_to_hold_fn_tcb() {
    assert_refused(
        &with_tls_edit("fn tcb() -> *mut", "fn read_tcb() -> *mut"),
        "G3",
    );
}

#[test]
fn a4_g1_refuses_an_off_manifest_name() {
    assert_refused(
        &with_extra(
            "overlay/std/net_neoplutarch.rs",
            "const X: u64 = 0; // wraps SYS_SPAWN\n",
        ),
        "G1",
    );
}

#[test]
fn a4_g2_refuses_an_allowed_name_bound_to_another_number() {
    // SYS_WRITE is allowed by name; binding it to 41 would issue syscall 41.
    assert_refused(
        &with_rt_edit("const SYS_WRITE: u64 = 1;", "const SYS_WRITE: u64 = 41;"),
        "G2",
    );
}

#[test]
fn a4_g3_refuses_a_second_syscall_site() {
    let rogue =
        "pub fn rogue() {\n    unsafe { core::arch::asm!(\"syscall\", options(nostack)); }\n}\n";
    assert_refused(&with_extra("overlay/std/fs_neoplutarch.rs", rogue), "G3");
}

#[test]
fn a4_g3_refuses_global_asm() {
    assert_refused(
        &with_extra(
            "overlay/std/x.rs",
            "core::arch::global_asm!(\".globl x\\nx: ret\");\n",
        ),
        "G3",
    );
}

#[test]
fn a4_g3_refuses_int80_and_sysenter() {
    let int80 = "fn f() { unsafe { core::arch::asm!(\"int 0x80\") } }\n";
    assert_refused(&with_extra("overlay/std/a.rs", int80), "G3");
    let sysenter = "fn g() { unsafe { core::arch::asm!(\"sysenter\") } }\n";
    assert_refused(&with_extra("overlay/std/b.rs", sysenter), "G3");
}

/// The bypass this rung closed: on `main` before C0.5 completion, the exit arm's
/// syscall lived in `apply-overlay.sh`, which the gate never scanned. Replayed
/// here as the script text it was; the gate now refuses it.
#[test]
fn a4_g7_refuses_syscall_code_in_an_overlay_script() {
    let legacy_exit_arm = "insert_arm \"$SYS/exit.rs\" 'pub fn exit(code: i32) -> ! {' \\\n\
        '    target_os = \"neoplutarch\" => {\n\
        unsafe {\n\
            core::arch::asm!(\n\
                \"syscall\",\n\
                in(\"rax\") 0u64,\n\
                in(\"rdi\") code as u64,\n\
            );\n\
        }\n\
    }' 'target_os = \"neoplutarch\"'\n";
    assert_refused(
        &with_extra("overlay/apply-overlay.sh", legacy_exit_arm),
        "G7",
    );
}

#[test]
fn a4_g4_refuses_a_literal_rax_inside_the_trampoline() {
    assert_refused(
        &with_rt_edit("in(\"rax\") num,", "in(\"rax\") 41u64,"),
        "G4",
    );
}

#[test]
fn a4_g4_refuses_inout_rax_outside_the_trampoline() {
    let rogue = "pub fn rogue(x: u64) -> u64 {\n    let y;\n    unsafe { core::arch::asm!(\"nop\", inout(\"rax\") x => y) };\n    y\n}\n";
    assert_refused(&with_extra("overlay/std/c.rs", rogue), "G4");
}

#[test]
fn a4_g5_refuses_a_literal_syscall_number() {
    assert_refused(&with_rt_edit("syscall4(SYS_EXIT, ", "syscall4(41, "), "G5");
}

#[test]
fn a4_g5_refuses_an_aliased_constant() {
    let s = with_rt_edit("syscall4(SYS_EXIT, ", "syscall4(EXIT_ALIAS, ");
    assert_refused(&s, "G5");
}

// ── 1b. the bypasses closed after the cross-family review (Chad, 2026-10-21) ──
//
// Chad's RETURN reported a method-qualified call (`Gate.syscall4(41, ..)`) next
// to a second `fn syscall4`. The old G5 already refused that exact call, but
// probing it found five real bypasses of the old checker (function pointer,
// multi-instruction template, a `"//"` string hiding the rest of the line, a
// block comment faking a trampoline, a second trampoline). Each is a committed
// fixture below, alongside the qualified-call and renamed-wrapper variants.

/// Chad's fixture verbatim: a second `fn syscall4` method + a qualified call.
const CHAD_FIXTURE: &str = "struct Gate;\nimpl Gate {\n    unsafe fn syscall4(num: u64, _a0: u64, _a1: u64, _a2: u64) -> i64 {\n        core::arch::asm!(\"syscall\", in(\"rax\") num, lateout(\"rax\") _);\n        0\n    }\n}\nfn rogue() { unsafe { Gate.syscall4(41, 0, 0, 0); } }\n";

#[test]
fn a4_chad_method_fixture_is_refused_by_g3_and_g5() {
    let s = with_extra("overlay/std/gate.rs", CHAD_FIXTURE);
    assert_refused(&s, "G3"); // the second trampoline and its asm
    assert_refused(&s, "G5"); // the literal at the call site
}

#[test]
fn a4_g5_refuses_every_qualified_call_form() {
    for call in [
        "Gate.syscall4(41, 0, 0, 0)",
        "Gate::syscall4(41, 0, 0, 0)",
        "Self::syscall4(41, 0, 0, 0)",
        "<Gate>::syscall4(41, 0, 0, 0)",
        "crate::sys::neoplutarch_rt::syscall4(41, 0, 0, 0)",
        "self.syscall4(41, 0, 0, 0)",
        "syscall4 (41, 0, 0, 0)",
    ] {
        let src = format!("fn rogue() {{ unsafe {{ {call}; }} }}\n");
        assert_refused(&with_extra("overlay/std/q.rs", &src), "G5");
    }
}

#[test]
fn a4_g5_refuses_a_function_pointer_and_an_alias() {
    let fnptr = "fn rogue() { let f = syscall4; unsafe { f(41, 0, 0, 0); } }\n";
    assert_refused(&with_extra("overlay/std/p.rs", fnptr), "G5");
    let alias = "use crate::sys::neoplutarch_rt::syscall4 as sc;\nfn rogue() { unsafe { sc(41, 0, 0, 0); } }\n";
    assert_refused(&with_extra("overlay/std/a.rs", alias), "G5");
    let turbofish = "fn rogue() { unsafe { syscall4::<u64>(41, 0, 0, 0); } }\n";
    assert_refused(&with_extra("overlay/std/t.rs", turbofish), "G5");
}

#[test]
fn a4_g5_refuses_a_renamed_wrapper_forwarding_its_number() {
    let wrapper =
        "pub fn sc(n: u64) -> i64 { unsafe { syscall4(n, 0, 0, 0) } }\nfn rogue() { sc(41); }\n";
    assert_refused(&with_extra("overlay/std/w.rs", wrapper), "G5");
}

#[test]
fn a4_g3_refuses_a_renamed_wrapper_holding_its_own_asm() {
    let wrapper = "unsafe fn sc(num: u64) -> i64 {\n    let r;\n    unsafe { core::arch::asm!(\"syscall\", in(\"rax\") num, lateout(\"rax\") r) };\n    r\n}\n";
    assert_refused(&with_extra("overlay/std/w.rs", wrapper), "G3");
}

#[test]
fn a4_g3_refuses_a_second_trampoline_even_with_named_constants() {
    let second = "const SYS_WRITE: u64 = 1;\nunsafe fn syscall4(num: u64) { unsafe { core::arch::asm!(\"syscall\", in(\"rax\") num) } }\nfn f() { unsafe { syscall4(SYS_WRITE) } }\n";
    assert_refused(&with_extra("overlay/std/s.rs", second), "G3");
}

#[test]
fn a4_g3_refuses_a_multi_instruction_template() {
    let outside =
        "fn rogue() { unsafe { core::arch::asm!(\"nop; syscall\", in(\"rdi\") 0u64); } }\n";
    assert_refused(&with_extra("overlay/std/m.rs", outside), "G3");
    // Inside the sanctioned trampoline, too: the template must be exactly "syscall".
    assert_refused(
        &with_rt_edit("\"syscall\",", "\"mov eax, 41; syscall\","),
        "G3",
    );
}

#[test]
fn a4_lexer_a_string_cannot_hide_the_rest_of_a_line() {
    let s = "fn rogue() { let _s = \"//\"; unsafe { syscall4(41, 0, 0, 0); } }\n";
    assert_refused(&with_extra("overlay/std/h.rs", s), "G5");
    let raw = "fn rogue() { let _s = r#\"\" // \"#; unsafe { syscall4(41, 0, 0, 0); } }\n";
    assert_refused(&with_extra("overlay/std/r.rs", raw), "G5");
}

#[test]
fn a4_lexer_a_comment_cannot_fake_a_trampoline() {
    let block = "/* fn syscall4 */ pub fn rogue(num: u64) { unsafe { core::arch::asm!(\"syscall\", in(\"rax\") num); } }\n";
    assert_refused(&with_extra("overlay/std/b.rs", block), "G3");
    let line = "// fn syscall4() {\npub fn rogue(num: u64) { unsafe { core::arch::asm!(\"syscall\", in(\"rax\") num); } }\n";
    assert_refused(&with_extra("overlay/std/l.rs", line), "G3");
}

#[test]
fn a4_g3_refuses_asm_emitted_by_a_macro() {
    let mac = "macro_rules! sc { ($n:expr) => { core::arch::asm!(\"syscall\", in(\"rax\") $n) } }\nfn rogue() { unsafe { sc!(41u64) } }\n";
    assert_refused(&with_extra("overlay/std/mac.rs", mac), "G3");
}

#[test]
fn a4_g2_refuses_rebinding_an_allowed_name() {
    let shadow =
        "fn rogue() { let SYS_WRITE: u64 = 41; unsafe { syscall4(SYS_WRITE, 0, 0, 0); } }\n";
    assert_refused(&with_extra("overlay/std/sh.rs", shadow), "G2");
    let stat =
        "static SYS_EXIT: u64 = 41;\nfn rogue() { unsafe { syscall4(SYS_EXIT, 0, 0, 0); } }\n";
    assert_refused(&with_extra("overlay/std/st.rs", stat), "G2");
    let import =
        "use libneo::SYS_WRITE;\nfn rogue() { unsafe { syscall4(SYS_WRITE, 0, 0, 0); } }\n";
    assert_refused(&with_extra("overlay/std/im.rs", import), "G2");
}

#[test]
fn a4_g6_refuses_side_doors() {
    for (name, src) in [
        ("include", "include!(\"../../elsewhere.rs\");\n"),
        ("path", "#[path = \"../../elsewhere.rs\"]\nmod elsewhere;\n"),
        (
            "extern block",
            "unsafe extern \"C\" { fn raw_syscall(n: u64) -> i64; }\n",
        ),
        ("link", "#[link(name = \"c\")]\nunsafe extern \"C\" {}\n"),
    ] {
        let s = with_extra(&format!("overlay/std/{}.rs", name.replace(' ', "_")), src);
        assert_refused(&s, "G6");
    }
}

#[test]
fn a4_g3_requires_each_trampoline_file_to_hold_its_trampoline() {
    assert_refused(
        &with_rt_edit("unsafe fn syscall4(", "unsafe fn trampoline("),
        "G3",
    );
}

// ── 2. io-errno table KAT ────────────────────────────────────────────────────

/// Independent hand-written expectation (the byte-oracle rule: not derived from
/// the table under test). LogansOS errno → `io::ErrorKind` variant name.
const EXPECTED_KINDS: &[(&str, i32, &str)] = &[
    ("EPERM", 1, "PermissionDenied"),
    ("ENOENT", 2, "NotFound"),
    ("ESRCH", 3, "NotFound"),
    ("EIO", 5, "Uncategorized"),
    ("E2BIG", 7, "ArgumentListTooLong"),
    ("EBADF", 9, "InvalidInput"),
    ("EAGAIN", 11, "WouldBlock"),
    ("ENOMEM", 12, "OutOfMemory"),
    ("EACCES", 13, "PermissionDenied"),
    ("EFAULT", 14, "InvalidInput"),
    ("EBUSY", 16, "ResourceBusy"),
    ("EEXIST", 17, "AlreadyExists"),
    ("ENOTDIR", 20, "NotADirectory"),
    ("EISDIR", 21, "IsADirectory"),
    ("EINVAL", 22, "InvalidInput"),
    ("EPIPE", 32, "BrokenPipe"),
    ("ENOSYS", 38, "Unsupported"),
    ("ENOTEMPTY", 39, "DirectoryNotEmpty"),
    ("EBADMSG", 74, "InvalidData"),
    ("ENOTCONN", 107, "NotConnected"),
    ("ETIMEDOUT", 110, "TimedOut"),
    ("EALREADY", 114, "AlreadyExists"),
    ("ESTALE", 116, "StaleNetworkFileHandle"),
    ("ECANCELED", 125, "Uncategorized"),
];

/// `pub const ENAME: i64 = -N;` from `kernel/src/errno.rs`.
fn kernel_errnos() -> BTreeMap<String, i32> {
    read("kernel/src/errno.rs")
        .lines()
        .filter_map(|l| {
            let rest = l.trim().strip_prefix("pub const E")?;
            let (name, rhs) = rest.split_once(": i64 = -")?;
            Some((
                format!("E{name}"),
                rhs.trim_end_matches(';').trim().parse().ok()?,
            ))
        })
        .collect()
}

/// `(N, "ENAME", ErrorKind::Kind, "msg"),` rows of the overlay's `ERRNO_TABLE`.
fn overlay_table() -> Vec<(i32, String, String)> {
    read("userland/neoplutarch-std-overlay/std/io_error_neoplutarch.rs")
        .lines()
        .filter_map(|l| {
            let row = l.trim().strip_prefix('(')?.strip_suffix("),")?;
            let mut cols = row.splitn(4, ", ");
            let n = cols.next()?.parse().ok()?;
            let name = cols.next()?.trim_matches('"').to_string();
            let kind = cols.next()?.strip_prefix("ErrorKind::")?.to_string();
            Some((n, name, kind))
        })
        .collect()
}

#[test]
fn errno_table_is_exhaustive_over_the_kernel_errnos() {
    let kernel = kernel_errnos();
    assert!(
        kernel.len() >= 20,
        "parsed only {} kernel errnos",
        kernel.len()
    );
    let table: BTreeMap<String, i32> = overlay_table()
        .into_iter()
        .map(|(n, name, _)| (name, n))
        .collect();
    assert_eq!(
        table, kernel,
        "overlay ERRNO_TABLE must name every kernel errno with its number"
    );
}

#[test]
fn errno_table_matches_the_independent_expectation() {
    let got: Vec<(String, i32, String)> = overlay_table()
        .into_iter()
        .map(|(n, name, kind)| (name, n, kind))
        .collect();
    let want: Vec<(String, i32, String)> = EXPECTED_KINDS
        .iter()
        .map(|&(name, n, kind)| (name.to_string(), n, kind.to_string()))
        .collect();
    assert_eq!(got, want);
}

#[test]
fn errno_table_never_maps_to_interrupted() {
    let src = read("userland/neoplutarch-std-overlay/std/io_error_neoplutarch.rs");
    assert!(!overlay_table().iter().any(|(_, _, k)| k == "Interrupted"));
    assert!(
        src.contains("pub fn is_interrupted(_code: i32) -> bool {\n    false\n}"),
        "is_interrupted must be constant false (no EINTR on LogansOS; std retry loops must not spin)"
    );
}

// ── 3. overlay consistency ───────────────────────────────────────────────────

#[test]
fn every_overlay_module_is_installed_by_apply_overlay() {
    let py = read("userland/neoplutarch-std-overlay/apply-overlay.py");
    let std_dir = root().join("userland/neoplutarch-std-overlay/std");
    let mut files = Vec::new();
    walk(&std_dir, &mut files);
    let mut modules: Vec<String> = files
        .iter()
        .map(|p| {
            p.strip_prefix(&std_dir)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect();
    modules.sort();
    assert_eq!(modules.len(), 12, "overlay modules: {modules:?}");
    for m in &modules {
        assert!(
            py.contains(&format!("\"{m}\": ")),
            "apply-overlay.py does not install std/{m}"
        );
        // xtask's post-apply byte check covers the same set.
        assert!(
            crate::OVERLAY_MODULES.iter().any(|(src, _)| src == m),
            "xtask OVERLAY_MODULES does not verify std/{m}"
        );
    }
    assert_eq!(crate::OVERLAY_MODULES.len(), modules.len());
    for (src, dst) in crate::OVERLAY_MODULES {
        assert!(
            py.contains(&format!("\"{src}\": \"{dst}\"")),
            "apply-overlay.py copies std/{src} somewhere other than sys/{dst}"
        );
    }
}

#[test]
fn abort_exit_code_agrees_between_overlay_and_kernel_scenario() {
    let rt = read("userland/neoplutarch-std-overlay/std/neoplutarch_rt.rs");
    assert!(rt.contains("pub const ABORT_EXIT_CODE: i32 = 134;"));
    assert!(
        read("kernel/src/test_harness.rs").contains("const STD_REAL_PANIC_EXIT_CODE: i32 = 134;")
    );
}

#[test]
fn elf_backstop_parses_syscall_sites_by_symbol() {
    let disasm = "\n0000000000201000 <neo_std_pal::syscall4>:\n  201000:      \tmov\trax, rdi\n  20103d:      \tsyscall\n\n0000000000202000 <evil::helper>:\n  202010:      \tint\t$0x80\n  202020:      \tsysenter\n  202030:      \tcall\t0x201000 <neo_std_pal::syscall4>\n";
    assert_eq!(
        crate::syscall_sites(disasm),
        [
            ("neo_std_pal::syscall4".to_string(), "syscall".to_string()),
            ("evil::helper".to_string(), "int $0x80".to_string()),
            ("evil::helper".to_string(), "sysenter".to_string()),
        ]
    );
}

#[test]
fn elf_backstop_parses_segment_relative_reads_by_symbol() {
    let disasm = "\n0000000000243800 <std::sys::neoplutarch_rt::tls::tcb>:\n  243801:      \tmovq\t%fs:0x0, %rax\n  243809:      \tretq\n\n0000000000250000 <evil::tls>:\n  250004:      \tmovq\t%gs:0x28, %rcx\n  250010:      \tmovq\t%rax, %rbx\n";
    assert_eq!(
        crate::segment_relative_sites(disasm),
        [
            (
                "std::sys::neoplutarch_rt::tls::tcb".to_string(),
                "movq %fs:0x0, %rax".to_string()
            ),
            ("evil::tls".to_string(), "movq %gs:0x28, %rcx".to_string()),
        ]
    );
}
