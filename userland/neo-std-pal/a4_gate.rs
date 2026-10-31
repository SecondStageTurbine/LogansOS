// SPDX-License-Identifier: MPL-2.0
//! OT-61 A4 — the PAL manifest gate's checker, as a pure function.
//!
//! Shared by `build.rs` (which fails the `neo-std-pal` build on any finding),
//! the committed host negative tests in `tools/xtask/src/ot61_c05_tests.rs`
//! (which prove each rule bites), and xtask's post-apply scan of the arms the
//! overlay inserts into rust-src. Dependency-free and `std`-only, so all three
//! can `#[path]`-include it.
//!
//! The scanned surface is every `.rs`, `.sh` and `.py` file under the PAL crate's
//! `src/` and under `userland/neoplutarch-std-overlay/` (the std overlay modules
//! AND the scripts that patch rust-src). Rust sources are lexed first: comments
//! are removed, and string/char literal contents are kept apart from code, so
//! neither a comment nor a `"//"` string can hide or fake code. Rules:
//!
//! - **G1 named.** Every `SYS_[A-Z]…` token anywhere in a file, comments and
//!   strings included, is in the manifest's `[gate].allowed_syscalls`.
//! - **G2 bindings.** In code, a `SYS_*` identifier appears in exactly two
//!   places: a `const SYS_X: <ty> = <integer literal>;` whose value is the
//!   kernel's number for `SYS_X` (`kernel/src/arch/x86_64/syscall.rs`), or the
//!   first argument of a `syscall4(` call. A call's constant must be declared
//!   in the same file. So `let`/`static`/`use … as` rebinding is refused.
//! - **G3 one trampoline, one TCB read.** The only sanctioned syscall site is a
//!   single `fn syscall4` (number + four arguments; a caller with fewer passes
//!   0) in each of the two trampoline files ([`TRAMPOLINE_FILES`]). Its body is
//!   found by brace matching. No other `fn syscall4` may exist anywhere. `asm!`
//!   may appear once inside that body, and its template must be exactly
//!   `"syscall"`. The one other sanctioned `asm!` (OT-61 C1.3) is the thread
//!   control block read: a single `fn tcb` in [`TCB_FILE`] whose body holds one
//!   `asm!` with the template exactly [`TCB_TEMPLATE`], a load that cannot
//!   issue a syscall. No other `fn tcb` may exist anywhere, and `asm!` is
//!   refused everywhere else. `global_asm!`, `naked_asm!` and `llvm_asm!` are
//!   refused everywhere.
//! - **G4 rax.** Inside the trampoline the only `rax` input is
//!   `in("rax") num`; `inout("rax")` is refused.
//! - **G5 call sites.** Every other occurrence of the identifier `syscall4` is
//!   a call (any qualification: `syscall4(`, `x.syscall4(`, `a::syscall4(`)
//!   whose first argument is a `SYS_*` constant. Any non-call reference (a
//!   function pointer, a `use … as` alias, a turbofish) is refused.
//! - **G6 no side doors.** `include!`, `#[path]`, `#[link…]`, and `extern`
//!   blocks (foreign declarations) are refused in scanned Rust sources.
//! - **G7 scripts.** A `.sh`/`.py` file may not mention a syscall or asm
//!   construct at all: `syscall`, `sysenter`, `asm!`, `_asm`, `core::arch`,
//!   `int 0x80` (case-insensitive), or the raw `0f 05` opcode bytes. Scripts
//!   patch rust-src; they never carry code that issues a syscall.
//!
//! Limits, stated so a reviewer can attack them: this is a lexical gate, not a
//! proof. It does not expand macros: a `macro_rules!` body is scanned like any
//! code, so a macro that emits `asm!` or calls `syscall4` is caught, but
//! token-pasting tricks are out of scope. Two non-lexical backstops in xtask
//! cover what lexing cannot. (1) The arms `apply-overlay.py` inserts into
//! rust-src are diffed against the pristine files and run through this
//! checker. (2) Every built std-real ELF is disassembled; each `syscall`,
//! `sysenter` or `int 0x80` instruction must sit inside one of the two
//! sanctioned `syscall4` symbols.

use std::collections::HashMap;

/// The two files allowed to hold the single `fn syscall4` trampoline (path
/// suffixes; `\` is normalised to `/`).
pub const TRAMPOLINE_FILES: &[&str] = &[
    "neo-std-pal/src/lib.rs",
    "neoplutarch-std-overlay/std/neoplutarch_rt.rs",
];

/// The file allowed to hold the single `fn tcb` (the shared TLS key layer,
/// which `apply-overlay.py` copies into std and libneo includes).
pub const TCB_FILE: &str = "neoplutarch-std-overlay/std/neoplutarch_rt/tls.rs";

/// The exact template of the `fn tcb` read: the thread control block's
/// self-pointer at `fs:[0]`.
pub const TCB_TEMPLATE: &str = "mov {}, qword ptr fs:[0]";

/// One scanned file: a display path plus its full text.
pub struct Source {
    pub path: String,
    pub text: String,
}

/// What the gate saw when it passed (the build-log evidence).
#[derive(Debug, PartialEq, Eq)]
pub struct Summary {
    pub referenced: Vec<String>,
    pub trampolines: usize,
    pub tcb_reads: usize,
}

/// Run every rule. `Err` carries one line per violation.
pub fn check(
    sources: &[Source],
    manifest: &str,
    kernel_table: &str,
) -> Result<Summary, Vec<String>> {
    let mut errs = Vec::new();
    let allowed = parse_allowed_syscalls(manifest);
    if allowed.is_empty() {
        return Err(vec![
            "manifest: [gate].allowed_syscalls is empty or unparsed".into(),
        ]);
    }
    let kernel = parse_kernel_syscall_table(kernel_table);
    if kernel.is_empty() {
        return Err(vec!["kernel syscall table parsed to 0 entries".into()]);
    }

    let mut referenced: Vec<String> = Vec::new();
    let mut trampolines = 0;
    let mut tcb_reads = 0;
    for src in sources {
        // G1 — over the raw text, comments and strings included.
        for tok in scan_sys_tokens(&src.text) {
            if !allowed.contains(&tok) {
                errs.push(format!(
                    "G1 {}: `{tok}` is not in [gate].allowed_syscalls",
                    src.path
                ));
            }
            if !referenced.contains(&tok) {
                referenced.push(tok);
            }
        }
        if src.path.ends_with(".rs") {
            let (t, r) = check_rust(src, &kernel, &mut errs);
            trampolines += t;
            tcb_reads += r;
        } else {
            check_script(src, &mut errs);
        }
    }

    referenced.sort();
    if errs.is_empty() {
        Ok(Summary {
            referenced,
            trampolines,
            tcb_reads,
        })
    } else {
        Err(errs)
    }
}

fn is_trampoline_file(path: &str) -> bool {
    let p = path.replace('\\', "/");
    TRAMPOLINE_FILES.iter().any(|t| p.ends_with(t))
}

fn is_tcb_file(path: &str) -> bool {
    path.replace('\\', "/").ends_with(TCB_FILE)
}

/// G2–G6 over one Rust source. Returns how many sanctioned trampolines and
/// `fn tcb` reads it holds.
fn check_rust(
    src: &Source,
    kernel: &HashMap<String, u64>,
    errs: &mut Vec<String>,
) -> (usize, usize) {
    let lex = Lexed::new(&src.text);
    let bare = lex.bare.as_str();
    let at = |pos: usize| format!("{}:{}", src.path, line_of(&src.text, pos));
    let mut found = 0;

    // G3 — locate the sanctioned trampoline (first `fn syscall4` in a
    // trampoline file) and its brace-matched body.
    let mut trampoline: Option<(usize, usize, usize)> = None; // (decl, body_start, body_end)
    for pos in word_positions(bare, "syscall4") {
        if !preceded_by_word(bare, pos, "fn") {
            continue;
        }
        if is_trampoline_file(&src.path) && trampoline.is_none() {
            match brace_body(bare, pos) {
                Some((s, e)) => {
                    trampoline = Some((pos, s, e));
                    found = 1;
                }
                None => errs.push(format!("G3 {}: `fn syscall4` has no body", at(pos))),
            }
        } else {
            errs.push(format!(
                "G3 {}: a second `fn syscall4` — only one sanctioned trampoline per trampoline file ({TRAMPOLINE_FILES:?})",
                at(pos)
            ));
        }
    }
    if is_trampoline_file(&src.path) && trampoline.is_none() {
        errs.push(format!(
            "G3 {}: trampoline file has no `fn syscall4`",
            src.path
        ));
    }
    let in_body = |pos: usize| trampoline.is_some_and(|(_, s, e)| pos > s && pos < e);

    // G3 — locate the sanctioned TCB read (the first `fn tcb` in the TCB file).
    let mut tcb: Option<(usize, usize)> = None;
    for pos in word_positions(bare, "tcb") {
        if !preceded_by_word(bare, pos, "fn") {
            continue;
        }
        if is_tcb_file(&src.path) && tcb.is_none() {
            match brace_body(bare, pos) {
                Some(body) => tcb = Some(body),
                None => errs.push(format!("G3 {}: `fn tcb` has no body", at(pos))),
            }
        } else {
            errs.push(format!(
                "G3 {}: a second `fn tcb` — only one sanctioned TCB read, in {TCB_FILE}",
                at(pos)
            ));
        }
    }
    if is_tcb_file(&src.path) && tcb.is_none() {
        errs.push(format!("G3 {}: the TCB file has no `fn tcb`", src.path));
    }
    let in_tcb = |pos: usize| tcb.is_some_and(|(s, e)| pos > s && pos < e);

    // G3 — asm macros.
    let mut asm_in_body = 0;
    let mut asm_in_tcb = 0;
    for name in ["asm", "global_asm", "naked_asm", "llvm_asm"] {
        for pos in word_positions(bare, name) {
            if !followed_by(bare, pos + name.len(), '!') {
                continue;
            }
            if name == "asm" && in_body(pos) {
                asm_in_body += 1;
                if asm_in_body > 1 {
                    errs.push(format!(
                        "G3 {}: a second `asm!` inside the trampoline",
                        at(pos)
                    ));
                }
                let template = lex.sole_template(pos);
                if template.as_deref() != Some("syscall") {
                    errs.push(format!(
                        "G3 {}: the trampoline's asm template must be exactly \"syscall\", got {template:?}",
                        at(pos)
                    ));
                }
            } else if name == "asm" && in_tcb(pos) {
                asm_in_tcb += 1;
                if asm_in_tcb > 1 {
                    errs.push(format!("G3 {}: a second `asm!` inside `fn tcb`", at(pos)));
                }
                let template = lex.sole_template(pos);
                if template.as_deref() != Some(TCB_TEMPLATE) {
                    errs.push(format!(
                        "G3 {}: the `fn tcb` asm template must be exactly {TCB_TEMPLATE:?}, got {template:?}",
                        at(pos)
                    ));
                }
            } else {
                errs.push(format!(
                    "G3 {}: `{name}!` outside the sanctioned `fn syscall4` trampoline and `fn tcb` read",
                    at(pos)
                ));
            }
        }
    }

    if let Some((s, _)) = tcb
        && asm_in_tcb == 0
    {
        errs.push(format!("G3 {}: `fn tcb` holds no `asm!` read", at(s)));
    }

    // G4 — rax operands. `code` keeps string contents, so `in("rax")` is
    // visible there; comments are already gone.
    let code = lex.code.as_str();
    for (pat, is_inout) in [("inout(\"rax\")", true), ("in(\"rax\")", false)] {
        let mut from = 0;
        while let Some(p) = code[from..].find(pat) {
            let pos = from + p;
            let operand: String = code[pos + pat.len()..]
                .trim_start()
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if is_inout || !(in_body(pos) && operand == "num") {
                errs.push(format!(
                    "G4 {}: rax operand `{}{operand}` — only `in(\"rax\") num` inside the trampoline is permitted",
                    at(pos),
                    if is_inout { "inout " } else { "" }
                ));
            }
            from = pos + pat.len();
        }
    }

    // G5 — every non-declaration `syscall4` is a call with a SYS_* first arg.
    let mut call_args: Vec<(usize, String)> = Vec::new();
    for pos in word_positions(bare, "syscall4") {
        if trampoline.is_some_and(|(d, _, _)| d == pos) || preceded_by_word(bare, pos, "fn") {
            continue; // the sanctioned decl, or a second decl already refused by G3
        }
        let after = pos + "syscall4".len();
        let rest = &bare[after..];
        let open = rest.len() - rest.trim_start().len();
        if !rest[open..].starts_with('(') {
            errs.push(format!(
                "G5 {}: `syscall4` referenced without a call — no function pointers or aliases",
                at(pos)
            ));
            continue;
        }
        let arg_start = after + open + 1;
        let arg = first_arg(&bare[arg_start..]);
        let trimmed = arg.trim();
        if is_sys_ident(trimmed) {
            let off = arg_start + (arg.len() - arg.trim_start().len());
            call_args.push((off, trimmed.to_string()));
        } else {
            errs.push(format!(
                "G5 {}: `syscall4({trimmed}, ..)` — the number must be a named SYS_* constant",
                at(pos)
            ));
        }
    }

    // G2 — SYS_* bindings in code.
    let mut declared: Vec<String> = Vec::new();
    for (pos, tok) in sys_positions(bare) {
        if call_args.iter().any(|(p, _)| *p == pos) {
            continue;
        }
        if preceded_by_word(bare, pos, "const") {
            match const_decl(&bare[pos..]) {
                Some(rhs) => match (parse_numeric_literal(rhs), kernel.get(&tok)) {
                    (Some(v), Some(&k)) if v == k => declared.push(tok),
                    (Some(v), Some(&k)) => errs.push(format!(
                        "G2 {}: `{tok}` = {v}, but the kernel defines {tok} = {k}",
                        at(pos)
                    )),
                    (_, None) => {
                        errs.push(format!("G2 {}: `{tok}` is not a kernel syscall", at(pos)))
                    }
                    (None, _) => errs.push(format!(
                        "G2 {}: `{tok}` must be bound to a plain integer literal, got `{rhs}`",
                        at(pos)
                    )),
                },
                None => errs.push(format!(
                    "G2 {}: malformed `const {tok}` declaration",
                    at(pos)
                )),
            }
        } else {
            errs.push(format!(
                "G2 {}: `{tok}` used outside a `const` declaration or a `syscall4(` first argument",
                at(pos)
            ));
        }
    }
    for (pos, tok) in &call_args {
        if !declared.contains(tok) {
            errs.push(format!(
                "G2 {}: `{tok}` is not declared by a checked `const` in this file",
                at(*pos)
            ));
        }
    }

    // G6 — side doors.
    for (needle, what) in [
        ("include!", "`include!` pulls in an unscanned file"),
        ("#[path", "`#[path]` points a module at an unscanned file"),
        ("#![path", "`#[path]` points a module at an unscanned file"),
        ("#[link", "`#[link…]` links foreign code"),
    ] {
        let mut from = 0;
        while let Some(p) = bare[from..].find(needle) {
            errs.push(format!("G6 {}: {what}", at(from + p)));
            from += p + needle.len();
        }
    }
    for pos in word_positions(bare, "extern") {
        let mut rest = bare[pos + "extern".len()..].trim_start();
        if rest.starts_with('"') {
            // `extern "ABI"` — the ABI string's contents are blanked in `bare`.
            if let Some(close) = rest[1..].find('"') {
                rest = rest[close + 2..].trim_start();
            }
        }
        if rest.starts_with('{') {
            errs.push(format!(
                "G6 {}: an `extern` block declares foreign functions",
                at(pos)
            ));
        }
    }
    (found, usize::from(tcb.is_some() && asm_in_tcb == 1))
}

/// G7 over one script.
fn check_script(src: &Source, errs: &mut Vec<String>) {
    for (n, line) in src.text.lines().enumerate() {
        let lower = line.to_ascii_lowercase();
        for needle in [
            "syscall",
            "sysenter",
            "asm!",
            "_asm",
            "core::arch",
            "int 0x80",
            "int $0x80",
            "\\x0f\\x05",
            "0f 05",
        ] {
            if lower.contains(needle) {
                errs.push(format!(
                    "G7 {}:{}: a script mentions `{needle}` — scripts patch rust-src and never carry syscall code",
                    src.path,
                    n + 1
                ));
            }
        }
    }
}

// ── lexing ───────────────────────────────────────────────────────────────────

/// Two same-length views of a Rust source: `code` has comments blanked
/// (newlines kept); `bare` additionally blanks string and char literal
/// contents (the quotes stay). Byte offsets agree across all three texts.
struct Lexed {
    code: String,
    bare: String,
}

impl Lexed {
    fn new(text: &str) -> Lexed {
        let b = text.as_bytes();
        let mut code = b.to_vec();
        let mut bare = b.to_vec();
        let blank = |v: &mut Vec<u8>, from: usize, to: usize| {
            for x in &mut v[from..to] {
                if *x != b'\n' {
                    *x = b' ';
                }
            }
        };
        let mut i = 0;
        while i < b.len() {
            let c = b[i];
            let prev_ident = i > 0 && is_ident(b[i - 1]);
            if c == b'/' && b.get(i + 1) == Some(&b'/') {
                let end = b[i..]
                    .iter()
                    .position(|&x| x == b'\n')
                    .map_or(b.len(), |p| i + p);
                blank(&mut code, i, end);
                blank(&mut bare, i, end);
                i = end;
            } else if c == b'/' && b.get(i + 1) == Some(&b'*') {
                let mut depth = 0;
                let mut j = i;
                while j < b.len() {
                    if b[j] == b'/' && b.get(j + 1) == Some(&b'*') {
                        depth += 1;
                        j += 2;
                    } else if b[j] == b'*' && b.get(j + 1) == Some(&b'/') {
                        depth -= 1;
                        j += 2;
                        if depth == 0 {
                            break;
                        }
                    } else {
                        j += 1;
                    }
                }
                blank(&mut code, i, j);
                blank(&mut bare, i, j);
                i = j;
            } else if (c == b'r' || (c == b'b' && b.get(i + 1) == Some(&b'r'))) && !prev_ident {
                // A raw string `r#"…"#` / `br#"…"#`, or just an identifier.
                let mut j = i + if c == b'b' { 2 } else { 1 };
                let mut hashes = 0;
                while b.get(j) == Some(&b'#') {
                    hashes += 1;
                    j += 1;
                }
                if b.get(j) == Some(&b'"') {
                    let body = j + 1;
                    let mut k = body;
                    let end = loop {
                        if k >= b.len() {
                            break b.len();
                        }
                        if b[k] == b'"'
                            && b[k + 1..]
                                .iter()
                                .take(hashes)
                                .filter(|&&x| x == b'#')
                                .count()
                                == hashes
                        {
                            break k;
                        }
                        k += 1;
                    };
                    blank(&mut bare, body, end);
                    i = (end + 1 + hashes).min(b.len());
                } else {
                    i += 1;
                    while i < b.len() && is_ident(b[i]) {
                        i += 1;
                    }
                }
            } else if c == b'"' {
                let body = i + 1;
                let mut k = body;
                while k < b.len() && b[k] != b'"' {
                    k += if b[k] == b'\\' { 2 } else { 1 };
                }
                let end = k.min(b.len());
                blank(&mut bare, body, end);
                i = end + 1;
            } else if c == b'\'' {
                // A char literal ('x', '\n', '\u{..}', multi-byte), or a lifetime.
                let close = if b.get(i + 1) == Some(&b'\\') {
                    b[i + 2..]
                        .iter()
                        .take(12)
                        .position(|&x| x == b'\'')
                        .map(|p| i + 2 + p)
                } else {
                    let len = text[i + 1..].chars().next().map_or(1, char::len_utf8);
                    (b.get(i + 1 + len) == Some(&b'\'')).then_some(i + 1 + len)
                };
                match close {
                    Some(end) => {
                        blank(&mut bare, i + 1, end);
                        i = end + 1;
                    }
                    None => i += 1,
                }
            } else if is_ident(c) {
                while i < b.len() && is_ident(b[i]) {
                    i += 1;
                }
            } else {
                i += 1;
            }
        }
        Lexed {
            code: String::from_utf8_lossy(&code).into_owned(),
            bare: String::from_utf8_lossy(&bare).into_owned(),
        }
    }

    /// The template of the `asm!` at `pos`: the contents of its first string
    /// literal, or `None` when a second template string follows it (`asm!`
    /// concatenates them, so `asm!("mov …", "syscall")` must not pass as the
    /// first one alone).
    fn sole_template(&self, pos: usize) -> Option<String> {
        let q = pos + self.bare[pos..].find('"')?;
        let close = q + 1 + self.bare[q + 1..].find('"')?;
        let next = self.bare[close + 1..].trim_start();
        if let Some(arg) = next.strip_prefix(',') {
            let arg = arg.trim_start();
            let raw = arg.strip_prefix('r').map(|r| r.trim_start_matches('#'));
            if arg.starts_with('"') || raw.is_some_and(|r| r.starts_with('"')) {
                return None;
            }
        }
        Some(self.code[q + 1..close].to_string())
    }
}

fn line_of(text: &str, pos: usize) -> usize {
    text.as_bytes()[..pos.min(text.len())]
        .iter()
        .filter(|&&b| b == b'\n')
        .count()
        + 1
}

/// Byte offsets of whole-word occurrences of `word` in `s`.
fn word_positions(s: &str, word: &str) -> Vec<usize> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(p) = s[from..].find(word) {
        let pos = from + p;
        let end = pos + word.len();
        if (pos == 0 || !is_ident(b[pos - 1])) && (end >= b.len() || !is_ident(b[end])) {
            out.push(pos);
        }
        from = end;
    }
    out
}

/// Whether the token before `pos` (skipping whitespace) is the word `w`.
fn preceded_by_word(s: &str, pos: usize, w: &str) -> bool {
    let before = s[..pos].trim_end();
    before.ends_with(w)
        && before.len() >= w.len()
        && (before.len() == w.len() || !is_ident(before.as_bytes()[before.len() - w.len() - 1]))
}

fn followed_by(s: &str, pos: usize, c: char) -> bool {
    s[pos..].trim_start().starts_with(c)
}

/// The brace-matched body `(open, close)` of the item declared at `pos`.
fn brace_body(s: &str, pos: usize) -> Option<(usize, usize)> {
    let open = pos + s[pos..].find('{')?;
    if s[pos..open].contains(';') {
        return None; // a bodiless declaration
    }
    let mut depth = 0;
    for (i, c) in s[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((open, open + i));
                }
            }
            _ => {}
        }
    }
    None
}

/// The text of a call's first argument (up to the first top-level `,` or `)`).
fn first_arg(s: &str) -> &str {
    let mut depth = 0i32;
    for (i, c) in s.char_indices() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' if depth == 0 => return &s[..i],
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => return &s[..i],
            _ => {}
        }
    }
    s
}

/// `SYS_X: <ty> = <rhs>;` at the start of `s` → `rhs`.
fn const_decl(s: &str) -> Option<&str> {
    let (head, rest) = s.split_once('=')?;
    if !head.contains(':') || head.contains(';') {
        return None;
    }
    let (rhs, _) = rest.split_once(';')?;
    Some(rhs.trim())
}

/// `(offset, token)` for every whole-word `SYS_[A-Z]…` identifier.
fn sys_positions(s: &str) -> Vec<(usize, String)> {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 4 < b.len() {
        if (i == 0 || !is_ident(b[i - 1]))
            && &b[i..i + 4] == b"SYS_"
            && b[i + 4].is_ascii_uppercase()
        {
            let mut j = i + 4;
            while j < b.len() && is_ident(b[j]) {
                j += 1;
            }
            out.push((i, s[i..j].to_string()));
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

/// Hand-parse `allowed_syscalls = ["SYS_A", "SYS_B", ...]` from the `[gate]`
/// table without a TOML dependency.
pub fn parse_allowed_syscalls(manifest: &str) -> Vec<String> {
    let mut in_gate = false;
    for line in manifest.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            in_gate = l == "[gate]";
            continue;
        }
        if in_gate && l.starts_with("allowed_syscalls") {
            let inside = l
                .split_once('[')
                .and_then(|(_, rest)| rest.rsplit_once(']'))
                .map(|(inner, _)| inner)
                .unwrap_or("");
            return inside
                .split(',')
                .map(|s| s.trim().trim_matches('"').to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
    }
    Vec::new()
}

/// `pub const SYS_XXX: u64 = VALUE;` lines from the kernel syscall table.
/// Feature-gated entries count too: the number space is global.
pub fn parse_kernel_syscall_table(src: &str) -> HashMap<String, u64> {
    let mut map = HashMap::new();
    for line in src.lines() {
        let l = line.trim();
        if let Some(rest) = l.strip_prefix("pub const ")
            && rest.starts_with("SYS_")
            && let Some(rhs) = const_decl(rest)
            && let Some(v) = parse_numeric_literal(rhs)
        {
            let name: String = rest
                .bytes()
                .take_while(|&b| is_ident(b))
                .map(char::from)
                .collect();
            map.insert(name, v);
        }
    }
    map
}

fn is_sys_ident(s: &str) -> bool {
    s.len() > 4
        && s.starts_with("SYS_")
        && s.as_bytes()[4].is_ascii_uppercase()
        && s.bytes().all(is_ident)
}

/// Extract `SYS_[A-Z][A-Z0-9_]*` identifier tokens (whole-word, deduplicated).
pub fn scan_sys_tokens(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (_, tok) in sys_positions(text) {
        if !out.contains(&tok) {
            out.push(tok);
        }
    }
    out
}

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// A Rust integer literal (decimal or `0x` hex, optional `_` separators and
/// `u64`/`usize`/`i64`/`u32` suffix).
fn parse_numeric_literal(s: &str) -> Option<u64> {
    let mut t = s.trim();
    for suffix in ["u64", "usize", "i64", "u32"] {
        t = t.strip_suffix(suffix).unwrap_or(t);
    }
    let t: String = t.chars().filter(|&c| c != '_').collect();
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).ok()
    } else if !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()) {
        t.parse().ok()
    } else {
        None
    }
}
