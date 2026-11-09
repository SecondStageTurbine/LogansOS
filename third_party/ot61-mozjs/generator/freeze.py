# SPDX-License-Identifier: MPL-2.0
"""OT-61 C3c-1: freeze what SpiderMonkey's build generated for our target.

Usage: freeze.py OBJ SRC REPO OUT [PATCHED PATCHES]

OBJ is the configured and exported object directory, SRC the patched source
root (`.../mozjs-sys/mozjs`), REPO the repository root, and OUT an empty
directory. OUT receives:

- `compile_commands.json`: one entry per object, from mozbuild's clangd
  database. Each entry has its directory, its source file and its compiler
  arguments, without `-o`, `-c` or the dependency-file flags.
- `obj/...`: every generated header and unified source the compiles read.
- `dist_include.txt`: `dist/include`'s links, one `name -> target` a line.
- `system_wrappers.txt`: the names in `dist/system_wrappers`. Every wrapper is
  the same four lines (checked here), so only the names are kept.
- `bindings/`: bindgen's `jsapi.rs` and `gluebindings.rs` for our target.
- `patched/...` (given PATCHED, the patched copy's root, and PATCHES, the
  patch directory): every file our patches change, whole, as patched, at its
  path under the vendored crate root. `patched/deleted.txt` lists the files
  they delete. The product build lays these over the vendored tree, and
  doesn't run `git apply`.

Absolute paths become `@SRC@`, `@OBJ@` and `@REPO@`, so the output doesn't
depend on where the runner put things. The product build substitutes its own
directories. The generator job diffs a fresh freeze against the committed
one, and fails on any difference.
"""

import json
import os
import re
import shlex
import sys

WRAPPER = (
    "#pragma GCC system_header\n"
    "#pragma GCC visibility push(default)\n"
    "#include_next <{name}>\n"
    "#pragma GCC visibility pop\n"
)

# Generated files the compiles read: headers, includes, unified sources.
GENERATED_SUFFIXES = (".h", ".inc", ".cpp", ".c", ".cc")
# Object-directory paths that are build plumbing, not compile inputs.
SKIP_PREFIXES = (
    "dist/",
    "clangd/",
    "_virtualenvs/",
    "_build_manifests/",
    "config/",
    "faster/",
)


def main():
    obj, src, repo, out = (os.path.realpath(a) for a in sys.argv[1:5])
    patched_root = os.path.realpath(sys.argv[5]) if len(sys.argv) > 6 else None
    patch_dir = os.path.realpath(sys.argv[6]) if len(sys.argv) > 6 else None
    roots = sorted(
        [(src, "@SRC@"), (obj, "@OBJ@"), (repo, "@REPO@")],
        key=lambda r: -len(r[0]),
    )

    def token(path):
        path = os.path.normpath(path)
        for root, tok in roots:
            if path == root:
                return tok
            if path.startswith(root + "/"):
                return tok + path[len(root) :]
        return path

    def tokenise_text(text):
        for root, tok in roots:
            text = text.replace(root + "/", tok + "/")
        return text

    os.makedirs(out, exist_ok=True)

    # The compile database.
    db = json.load(open(os.path.join(obj, "clangd", "compile_commands.json")))
    entries = []
    for e in db:
        d = e["directory"]
        args = shlex.split(e["command"]) if "command" in e else list(e["arguments"])
        kept = []
        i = 0
        while i < len(args):
            a = args[i]
            if a in ("-o", "-MF"):
                i += 2
                continue
            if a in ("-c", "-MD", "-MP") or "$(MDDEPDIR)" in a:
                i += 1
                continue
            if a in ("-include", "-isystem", "-iquote", "-I") and i + 1 < len(args):
                p = args[i + 1]
                p = p if os.path.isabs(p) else os.path.join(d, p)
                kept += [a, token(p)]
                i += 2
                continue
            if a.startswith("-I") and len(a) > 2:
                p = a[2:]
                p = p if os.path.isabs(p) else os.path.join(d, p)
                kept.append("-I" + token(p))
                i += 1
                continue
            if i == 0 or os.path.isabs(a):
                kept.append(token(a))
            elif not a.startswith("-") and os.path.exists(os.path.join(d, a)):
                kept.append(token(os.path.join(d, a)))
            else:
                kept.append(a)
            i += 1
        f = e["file"] if os.path.isabs(e["file"]) else os.path.join(d, e["file"])
        entries.append({"directory": token(d), "file": token(f), "arguments": kept})
    # mozbuild's clangd database lists every source on its own, but make
    # compiles unified files (the build the compile probe proved). One
    # entry per object: each Unified_*.cpp with its members' flags, and
    # every source no unified file includes, as it is.
    by_file = {e["file"]: e for e in entries}
    unified = []
    members = set()
    inc_line = re.compile(r'^#include "([^"]+)"$', re.M)
    for dirpath, dirnames, filenames in os.walk(obj):
        for name in filenames:
            if not (name.startswith("Unified_") and name.endswith((".cpp", ".c"))):
                continue
            path = os.path.join(dirpath, name)
            srcs = [
                token(p if os.path.isabs(p) else os.path.join(dirpath, p))
                for p in inc_line.findall(open(path, encoding="utf-8").read())
            ]
            known = [s for s in srcs if s in by_file]
            if not known:
                sys.exit(f"{path}: none of its sources is in the compile database")
            members.update(srcs)
            t = by_file[known[0]]
            args = [token(path) if a == known[0] else a for a in t["arguments"]]
            unified.append({"directory": token(dirpath), "file": token(path), "arguments": args})
    entries = sorted(
        unified + [e for e in entries if e["file"] not in members],
        key=lambda e: e["file"],
    )
    objects = [
        f
        for dp, _, fs in os.walk(obj)
        for f in fs
        if f.endswith(".o")
        and not f.startswith(("host_", "conftest"))
        # The bindgen step's own compiles (cc-rs names them <hash>-jsapi.o).
        and not f.endswith(("jsapi.o", "jsglue.o"))
    ]
    print(f"freeze: {len(unified)} unified files; {len(objects)} objects built")
    if len(entries) != len(objects):
        print(f"freeze: {len(entries)} compile entries but {len(objects)} objects built")
    with open(os.path.join(out, "compile_commands.json"), "w") as fh:
        json.dump(entries, fh, indent=1, sort_keys=True)
        fh.write("\n")

    # Generated files, with their includes of sources made relocatable.
    include = re.compile(r'"([^"\n]+\.(?:cpp|cc|c|h))"')
    n_gen = 0
    for dirpath, dirnames, filenames in os.walk(obj):
        dirnames.sort()
        rel_dir = os.path.relpath(dirpath, obj)
        for name in sorted(filenames):
            rel = os.path.normpath(os.path.join(rel_dir, name))
            if rel.startswith(SKIP_PREFIXES) or not name.endswith(GENERATED_SUFFIXES):
                continue
            path = os.path.join(dirpath, name)
            # conftest*: configure's probes; flag_check*: cc-rs's (the
            # bindgen step's jsapi/jsglue compile).
            if os.path.islink(path) or name.startswith(("conftest", "flag_check")):
                continue
            text = open(path, encoding="utf-8").read()

            moved = {}

            def relocate(m):
                p = m.group(1)
                if os.path.isabs(p):
                    moved[p] = token(p)
                    return '"' + token(p) + '"'
                full = os.path.normpath(os.path.join(dirpath, p))
                if os.path.exists(full) and not full.startswith(obj + "/"):
                    moved[p] = token(full)
                    return '"' + token(full) + '"'
                return m.group(0)

            text = include.sub(relocate, text)
            # The same paths also appear inside longer strings (a unified
            # file's #error messages name the source it includes).
            for old, new in sorted(moved.items(), key=lambda kv: -len(kv[0])):
                text = text.replace(old, new)
            text = tokenise_text(text)
            dest = os.path.join(out, "obj", rel)
            os.makedirs(os.path.dirname(dest), exist_ok=True)
            with open(dest, "w", encoding="utf-8") as fh:
                fh.write(text)
            n_gen += 1

    # Bindgen's output (the generator's bindgen step): upstream's
    # jsapi.rs and gluebindings.rs for our target.
    os.makedirs(os.path.join(out, "bindings"), exist_ok=True)
    for name in ("jsapi.rs", "gluebindings.rs"):
        path = os.path.join(obj, name)
        if not os.path.exists(path):
            sys.exit(f"{name} is missing: run the bindgen step before freezing")
        text = tokenise_text(open(path, encoding="utf-8").read())
        with open(os.path.join(out, "bindings", name), "w", encoding="utf-8") as fh:
            fh.write(text)

    # The files our patches touch, whole and as patched.
    if patched_root:
        changed, deleted = set(), set()
        for name in sorted(os.listdir(patch_dir)):
            if not name.endswith(".patch"):
                continue
            minus = None
            for line in open(os.path.join(patch_dir, name), encoding="utf-8"):
                if line.startswith("--- "):
                    minus = line[4:].strip()
                elif line.startswith("+++ "):
                    plus = line[4:].strip()
                    if plus == "/dev/null":
                        deleted.add(minus[2:])
                    else:
                        changed.add(plus[2:])
        for rel in sorted(changed - deleted):
            text = open(os.path.join(patched_root, rel), encoding="utf-8").read()
            dest = os.path.join(out, "patched", rel)
            os.makedirs(os.path.dirname(dest), exist_ok=True)
            with open(dest, "w", encoding="utf-8") as fh:
                fh.write(text)
        with open(os.path.join(out, "patched", "deleted.txt"), "w") as fh:
            fh.write("".join(f"{d}\n" for d in sorted(deleted)))
        print(f"freeze: {len(changed - deleted)} patched files, {len(deleted)} deleted")

    # The jsapi.cpp/jsglue.cpp compiles the bindgen step ran through neo-cc
    # (NEO_CC_LOG, copied to obj/glue-cc.log): upstream get_common_cc's
    # flags, recorded, so the product build never re-derives them. cc-rs's
    # flag probes are dropped; only the two sources are kept.
    glue_log = os.path.join(obj, "glue-cc.log")
    if os.path.exists(glue_log):
        glue = {}
        for line in open(glue_log, encoding="utf-8"):
            e = json.loads(line)
            src = [a for a in e["arguments"] if a.endswith(("src/jsapi.cpp", "src/jsglue.cpp"))]
            if not src:
                continue
            args = []
            skip = False
            for i, a in enumerate(e["arguments"]):
                if skip:
                    skip = False
                    continue
                if a == "-o":
                    skip = True
                    continue
                if a == "-c":
                    continue
                p = a
                if a.startswith("-I") and len(a) > 2:
                    p = a[2:]
                    p = p if os.path.isabs(p) else os.path.join(e["directory"], p)
                    args.append("-I" + token(p))
                    continue
                if os.path.isabs(a) or a.startswith("./"):
                    args.append(token(os.path.normpath(os.path.join(e["directory"], a))))
                    continue
                args.append(a)
            name = os.path.basename(src[0])
            glue[name] = {"cxx": e["cxx"], "file": name, "arguments": args}
        if sorted(glue) != ["jsapi.cpp", "jsglue.cpp"]:
            sys.exit(f"glue-cc.log has {sorted(glue)}, want jsapi.cpp and jsglue.cpp")
        with open(os.path.join(out, "glue_commands.json"), "w") as fh:
            json.dump([glue[k] for k in sorted(glue)], fh, indent=1, sort_keys=True)
            fh.write("\n")
        print("freeze: glue_commands.json (jsapi.cpp, jsglue.cpp)")

    # dist/include: links only.
    links = []
    base = os.path.join(obj, "dist", "include")
    for dirpath, dirnames, filenames in os.walk(base):
        for name in filenames:
            p = os.path.join(dirpath, name)
            rel = os.path.relpath(p, base)
            if not os.path.islink(p):
                sys.exit(f"dist/include/{rel} is a file, not a link")
            links.append(f"{rel} -> {token(os.path.realpath(p))}")
    links.sort()
    with open(os.path.join(out, "dist_include.txt"), "w") as fh:
        fh.write("\n".join(links) + "\n")

    # dist/system_wrappers: one template.
    names = []
    base = os.path.join(obj, "dist", "system_wrappers")
    for dirpath, dirnames, filenames in os.walk(base):
        for name in filenames:
            p = os.path.join(dirpath, name)
            rel = os.path.relpath(p, base)
            if open(p).read() != WRAPPER.format(name=rel):
                sys.exit(f"system_wrappers/{rel} isn't the standard wrapper")
            names.append(rel)
    names.sort()
    with open(os.path.join(out, "system_wrappers.txt"), "w") as fh:
        fh.write("\n".join(names) + "\n")

    leaks = [
        os.path.join(dp, f)
        for dp, _, fs in os.walk(out)
        for f in fs
        if any(r in open(os.path.join(dp, f), encoding="utf-8").read() for r, _ in roots)
    ]
    print(
        f"freeze: {len(entries)} compile entries, {n_gen} generated files, "
        f"{len(links)} dist/include links, {len(names)} system wrappers"
    )
    if leaks:
        for p in leaks:
            print(f"freeze: an absolute path survived in {p}")
        sys.exit(1)


if __name__ == "__main__":
    main()
