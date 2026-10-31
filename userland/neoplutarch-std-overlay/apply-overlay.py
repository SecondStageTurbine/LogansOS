#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
#
# OT-61 C0.5 — apply the bounded `neoplutarch` std overlay to the active
# toolchain's rust-src, so `-Z build-std=std` compiles a real `std` for
# x86_64-unknown-neoplutarch. Invoked by apply-overlay.sh (which xtask runs,
# only for the std_real_smoke scenario).
#
# What it does, in order:
#   1. RESTORE every file it patches from a pristine backup (`<file>.neo-pristine`),
#      creating the backup on first run. A rust-src patched by the pre-backup
#      overlay (no backup, `neoplutarch` already present) is reverted
#      structurally first. So every apply starts from pristine rust-src and an
#      edited arm can never be silently skipped as "already patched".
#   2. COPY the overlay modules (std/*.rs) into sys/.
#   3. PATCH the cfg_select! seams (each arm only names a module or calls into
#      sys::neoplutarch_rt — no arm carries kernel-call code of its own, so the A4
#      gate in userland/neo-std-pal/a4_gate.rs sees the whole kernel-call surface
#      in std/*.rs).
#   4. AUDIT every sys/* module the overlay does NOT touch: its `_ =>` fallback
#      must still be the loud `unsupported` (or single-threaded `no_threads`)
#      module. A toolchain bump that changes a fallback fails here, loudly.
#
# Seams patched (everything else stays std's loud `unsupported` stub tier):
#   sys/mod.rs                 declare `neoplutarch_rt` (the single kernel-call seam)
#   sys/stdio/mod.rs           -> stdio/neoplutarch.rs   (stdout/stderr write)
#   sys/exit.rs (fn exit)      -> neoplutarch_rt::exit   (clean exit)
#   sys/pal/unsupported/common.rs abort_internal -> neoplutarch_rt::abort (exit 134)
#   sys/io/error/mod.rs        -> io/error/neoplutarch.rs (the errno table)
#   sys/alloc/mod.rs           -> alloc/neoplutarch.rs   (System allocator)
#   sys/random/mod.rs          -> random/neoplutarch.rs fill_bytes (C2.0; SYS_GETRANDOM,
#                                 and without kernel-rng it panics => abort 134)
#   sys/thread_local/mod.rs    -> the OS-key route on neoplutarch_rt/tls.rs (C1.2/C1.3)
#   sys/mod.rs (futex)         -> neoplutarch_rt::futex (C1.3)
#   sys/sync/{mutex,condvar,rwlock,once,thread_parking}/mod.rs -> std's futex
#                                 implementations (C1.3)
#   sys/thread/mod.rs          -> thread/neoplutarch.rs  (std::thread, C1.3)
#   sys/time/mod.rs            -> time/neoplutarch.rs Instant + unsupported
#                                 SystemTime (C1.3)
# A1: nothing here maps executable memory.
import os
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
MARK = 'target_os = "neoplutarch"'


def die(msg):
    sys.stderr.write(f"apply-overlay: FAILED: {msg}\n")
    sys.exit(1)


def sysroot():
    out = subprocess.run(["rustc", "--print", "sysroot"], capture_output=True, text=True)
    if out.returncode != 0:
        die(f"rustc --print sysroot: {out.stderr.strip()}")
    return out.stdout.strip()


# xtask passes a per-build copy of the toolchain's `library/` (NEO_STD_LIBRARY)
# and points -Zbuild-std at it, so the shared toolchain is never patched and a
# std build cannot see what another checkout or runner job applied. Without it
# (a manual run) the toolchain's own rust-src is patched, as before.
LIBRARY = os.environ.get("NEO_STD_LIBRARY") or os.path.join(
    sysroot(), "lib", "rustlib", "src", "rust", "library")
SYS = os.path.join(LIBRARY, "std", "src", "sys")
if not os.path.isdir(SYS):
    die(f"rust-src not at {SYS}")
print(f"apply-overlay: rust-src = {SYS}")


def path(rel):
    return os.path.join(SYS, rel)


def read(rel):
    with open(path(rel), encoding="utf-8") as f:
        return f.read()


def write(rel, text):
    with open(path(rel), "w", encoding="utf-8") as f:
        f.write(text)


def strip_legacy(text):
    """Structurally remove a pre-backup overlay: every `target_os = "neoplutarch" => {...}`
    arm (with its leading newline + indent) and every `target_os = "neoplutarch",`
    list entry line."""
    out = text
    while True:
        i = out.find(MARK + " => {")
        if i < 0:
            break
        # Extend left over the indent and the newline the old script inserted.
        start = i
        while start > 0 and out[start - 1] == " ":
            start -= 1
        if start > 0 and out[start - 1] == "\n":
            start -= 1
        j = out.index("{", i)
        depth = 0
        k = j
        while k < len(out):
            if out[k] == "{":
                depth += 1
            elif out[k] == "}":
                depth -= 1
                if depth == 0:
                    break
            k += 1
        else:
            die("legacy overlay arm has unbalanced braces")
        out = out[:start] + out[k + 1:]
    lines = [l for l in out.split("\n") if l.strip() != MARK + ","]
    return "\n".join(lines)


PATCHED = [
    "mod.rs",
    "stdio/mod.rs",
    "exit.rs",
    "pal/unsupported/common.rs",
    "io/error/mod.rs",
    "alloc/mod.rs",
    "random/mod.rs",
    "thread_local/mod.rs",
    "sync/mutex/mod.rs",
    "sync/condvar/mod.rs",
    "sync/rwlock/mod.rs",
    "sync/once/mod.rs",
    "sync/thread_parking/mod.rs",
    "thread/mod.rs",
    "time/mod.rs",
    "fs/mod.rs",
    "env/mod.rs",
    "process/unsupported.rs",
    "../os/mod.rs",
]

# 1. restore pristine
for rel in PATCHED:
    backup = path(rel) + ".neo-pristine"
    if os.path.exists(backup):
        shutil.copyfile(backup, path(rel))
        continue
    text = read(rel)
    if "neoplutarch" in text:
        text = strip_legacy(text)
        if "neoplutarch" in text:
            die(f"{rel}: carries a neoplutarch patch this script cannot revert; "
                "reinstall rust-src (rustup component remove rust-src && rustup component add rust-src)")
        write(rel, text)
        print(f"apply-overlay: reverted legacy (pre-backup) patch in {rel}")
    shutil.copyfile(path(rel), backup)
# rust-src is shared by every checkout on the machine (and by a self-hosted
# runner's jobs), so it may carry another overlay version's patches. Restore
# every backup found, not only this version's list, so a seam patched there
# and not here goes back to pristine instead of tripping the routing audit.
STD = os.path.dirname(SYS)
for root, _dirs, files in os.walk(STD):
    for f in files:
        if f.endswith(".neo-pristine"):
            target = os.path.join(root, f[: -len(".neo-pristine")])
            rel = os.path.normpath(os.path.relpath(target, SYS))
            if rel not in {os.path.normpath(r) for r in PATCHED}:
                shutil.copyfile(os.path.join(root, f), target)
                print(f"apply-overlay: restored {rel} (patched by another overlay version)")
print("apply-overlay: restored pristine rust-src for the patched seams")

# 2. copy the overlay modules
COPIES = {
    "neoplutarch_rt.rs": "neoplutarch_rt.rs",
    "stdio_neoplutarch.rs": "stdio/neoplutarch.rs",
    "alloc_neoplutarch.rs": "alloc/neoplutarch.rs",
    "io_error_neoplutarch.rs": "io/error/neoplutarch.rs",
    "tls_key_neoplutarch.rs": "thread_local/key/neoplutarch.rs",
    "neoplutarch_rt/tls.rs": "neoplutarch_rt/tls.rs",
    "thread_neoplutarch.rs": "thread/neoplutarch.rs",
    "time_neoplutarch.rs": "time/neoplutarch.rs",
    "random_neoplutarch.rs": "random/neoplutarch.rs",
    "fs_neoplutarch.rs": "fs/neoplutarch.rs",
    "env_neoplutarch.rs": "env/neoplutarch.rs",
    "os_neoplutarch.rs": "../os/neoplutarch/mod.rs",
}
# Remove overlay modules this version does not ship (left by another version).
wanted = {os.path.normpath(d) for d in COPIES.values()}
for root, dirs, files in os.walk(STD, topdown=False):
    for f in files:
        full = os.path.join(root, f)
        rel = os.path.normpath(os.path.relpath(full, SYS))
        if "neoplutarch" in rel and not f.endswith(".neo-pristine") and rel not in wanted:
            os.remove(full)
            print(f"apply-overlay: removed {rel} (not in this overlay)")
    for d in dirs:
        full = os.path.join(root, d)
        if "neoplutarch" in d and not os.listdir(full):
            os.rmdir(full)
for src, dst in COPIES.items():
    os.makedirs(os.path.dirname(path(dst)), exist_ok=True)
    shutil.copyfile(os.path.join(HERE, "std", src), path(dst))
print(f"apply-overlay: copied {len(COPIES)} overlay modules")


# 3. patch the seams
def insert_arm(rel, anchor, arm):
    """Insert `arm` as the first arm of the first `cfg_select! {` after `anchor`."""
    src = read(rel)
    start = src.find(anchor) if anchor else 0
    if start < 0:
        die(f"anchor {anchor!r} not in {rel} (toolchain changed?)")
    i = src.find("cfg_select! {", start)
    if i < 0:
        die(f"no cfg_select! after anchor in {rel}")
    j = i + len("cfg_select! {")
    write(rel, src[:j] + "\n" + arm + src[j:])
    print(f"apply-overlay: patched {rel}")


def replace_once(rel, old, new):
    src = read(rel)
    if src.count(old) != 1:
        die(f"{rel}: expected exactly one {old!r} (found {src.count(old)}; toolchain changed?)")
    write(rel, src.replace(old, new))
    print(f"apply-overlay: patched {rel}")


replace_once("mod.rs", "pub mod time;\n",
             "pub mod time;\n#[cfg(target_os = \"neoplutarch\")]\npub(crate) mod neoplutarch_rt;\n"
             "#[cfg(target_os = \"neoplutarch\")]\npub(crate) use neoplutarch_rt::futex;\n")

insert_arm("stdio/mod.rs", "", f"""    {MARK} => {{
        mod neoplutarch;
        pub use neoplutarch::*;
    }}""")

insert_arm("exit.rs", "pub fn exit(code: i32) -> ! {", f"""        {MARK} => {{
            crate::sys::neoplutarch_rt::exit(code)
        }}""")

replace_once("pal/unsupported/common.rs",
             "pub fn abort_internal() -> ! {\n    core::intrinsics::abort();\n}",
             "pub fn abort_internal() -> ! {\n"
             "    #[cfg(target_os = \"neoplutarch\")]\n"
             "    crate::sys::neoplutarch_rt::abort();\n"
             "    #[cfg(not(target_os = \"neoplutarch\"))]\n"
             "    core::intrinsics::abort();\n}")

insert_arm("io/error/mod.rs", "", f"""    {MARK} => {{
        mod neoplutarch;
        pub use neoplutarch::*;
    }}""")

insert_arm("alloc/mod.rs", "", f"""    {MARK} => {{
        mod neoplutarch;
    }}""")

# random (C2.0): std's fill_bytes on the kernel CSPRNG. std's generic
# hashmap_random_keys (outside the select) builds on it, so HashMap's
# RandomState works. A kernel without kernel-rng makes it panic => abort 134,
# the same outcome as the C0.5 unsupported route it replaces.
insert_arm("random/mod.rs", "", f"""    {MARK} => {{
        mod neoplutarch;
        pub use neoplutarch::fill_bytes;
    }}""")

# thread_local (C1.2 key-only spike): leave neoplutarch OFF the no_threads lists,
# so std takes its `os` key-based mechanism and the `guard/key.rs` cleanup, and
# give the `key` select a neoplutarch arm (racy LazyKey + the overlay key module).
# Matches the target JSON (singlethread=false, has_thread_local=false): OS keys,
# no PT_TLS.
insert_arm("thread_local/mod.rs", "pub(crate) mod key {", f"""        {MARK} => {{
            mod racy;
            mod neoplutarch;
            pub(super) use racy::LazyKey;
            pub(super) use neoplutarch::{{Key, get, set}};
            use neoplutarch::{{create, destroy}};
        }}""")
print("apply-overlay: routed thread_local to the OS-key route (key arm)")

# C1.3: std's futex-based sync primitives (the same modules Linux, hermit and
# motor use) over sys::futex = neoplutarch_rt::futex.
for rel, items in [
    ("sync/mutex/mod.rs", "Mutex"),
    ("sync/condvar/mod.rs", "Condvar"),
    ("sync/rwlock/mod.rs", "RwLock"),
    ("sync/once/mod.rs", "{Once, OnceState}"),
    ("sync/thread_parking/mod.rs", "Parker"),
]:
    insert_arm(rel, "", f"""    {MARK} => {{
        mod futex;
        pub use futex::{items};
    }}""")

# C1.3: std::thread on the C1 thread calls.
insert_arm("thread/mod.rs", "", f"""    {MARK} => {{
        mod neoplutarch;
        pub use neoplutarch::{{Thread, available_parallelism, current_os_id, set_name, sleep, yield_now, DEFAULT_MIN_STACK_SIZE}};
    }}""")

# C1.3: Instant on the monotonic clock; SystemTime stays unsupported (see
# time_neoplutarch.rs).
insert_arm("time/mod.rs", "", f"""    {MARK} => {{
        mod neoplutarch;
        #[expect(unused)]
        mod unsupported;
        mod imp {{
            pub use super::neoplutarch::Instant;
            pub use super::unsupported::{{SystemTime, UNIX_EPOCH}};
        }}
    }}""")

# C2.1b: std::fs over the process's own file capabilities (fail-closed without
# a RootDirCap), an empty immutable environment, process::id via
# SYS_PROCESS_SELF, and std::os::neoplutarch (raw handles, the handle table).
insert_arm("fs/mod.rs", "", f"""    {MARK} => {{
        mod neoplutarch;
        use neoplutarch as imp;
    }}""")

insert_arm("env/mod.rs", "", f"""    {MARK} => {{
        mod neoplutarch;
        pub use neoplutarch::*;
    }}""")

replace_once("process/unsupported.rs",
             "pub fn getpid() -> u32 {\n    panic!(\"no pids on this platform\")\n}",
             "pub fn getpid() -> u32 {\n"
             "    #[cfg(target_os = \"neoplutarch\")]\n"
             "    {\n"
             "        let pid = crate::sys::neoplutarch_rt::process_self();\n"
             "        if pid > 0 {\n"
             "            return pid as u32;\n"
             "        }\n"
             "    }\n"
             "    panic!(\"no pids on this platform\")\n}")

replace_once("../os/mod.rs", "#[cfg(target_os = \"motor\")]\npub mod motor;\n",
             "#[cfg(target_os = \"motor\")]\npub mod motor;\n"
             "#[cfg(target_os = \"neoplutarch\")]\npub mod neoplutarch;\n")

# 4. audit the untouched fallbacks: (file, module the `_ =>` arm must name)
FALLBACKS = [
    ("args/mod.rs", "mod unsupported"),
    ("net/connection/mod.rs", "mod unsupported"),
    ("process/mod.rs", "mod unsupported"),
    ("pipe/mod.rs", "mod unsupported"),
    ("paths/mod.rs", "mod unsupported"),
    ("pal/mod.rs", "mod unsupported"),
]
for rel, want in FALLBACKS:
    src = read(rel)
    if "neoplutarch" in src:
        die(f"{rel}: names neoplutarch, but the overlay does not route it (toolchain changed?)")
    i = src.rfind("_ => {")
    if i < 0:
        die(f"{rel}: no `_ =>` fallback arm (toolchain changed?)")
    arm = src[i:src.find("}", i)]
    if want not in arm:
        die(f"{rel}: `_ =>` fallback no longer `{want}` (toolchain changed?): {arm!r}")
print(f"apply-overlay: audited {len(FALLBACKS)} untouched sys/* fallbacks (all loud unsupported)")
print("apply-overlay: done.")
