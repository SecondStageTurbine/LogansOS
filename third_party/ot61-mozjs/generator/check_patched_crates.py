# SPDX-License-Identifier: MPL-2.0
"""OT-61 C3c-1: check the product build's patched crate copies.

Usage: check_patched_crates.py REPO

`third_party/ot61-mozjs/patched/` holds mozjs, mozjs-sys and normalizer_glue
for the product build (docs/THINK-2026-11-08-ot61-c3c1-product-build.md §2).
They may differ from the vendored upstream crates only in their manifests and
mozjs-sys's build.rs. This checks that:

- each copy holds only `Cargo.toml` (and `build.rs` for mozjs-sys), so no
  source of its own;
- its library is the vendored crate's own `src/lib.rs` (`[lib] path`);
- its name, version, licence and edition are upstream's;
- it has no `[build-dependencies]`, and only mozjs-sys has a build script;
- its normal dependencies are upstream's, less the optional ones its removed
  features used, with the same version requirements.
"""

import os
import sys
import tomllib

VENDOR = "third_party/ot61-mozjs/vendor/mozjs-138c9470"
PATCHED = "third_party/ot61-mozjs/patched"

# copy -> (upstream crate dir under VENDOR, files allowed, deps dropped)
CRATES = {
    "mozjs": ("mozjs", {"Cargo.toml"}, set()),
    "mozjs-sys": (
        "mozjs-sys",
        {"Cargo.toml", "build.rs"},
        {
            "icu_capi",
            "mozjs_unicode_bidi_ffi",
            "mozjs_properties_glue",
            "mozjs_collator_glue",
            "mozjs_locale_glue",
            "libz-rs-sys",
        },
    ),
    "normalizer_glue": ("mozjs-extracted-crates/normalizer_glue", {"Cargo.toml"}, set()),
}


def load(path):
    with open(path, "rb") as fh:
        return tomllib.load(fh)


def field(pkg, ws, key):
    v = pkg.get(key)
    if isinstance(v, dict) and v.get("workspace"):
        return ws["package"][key]
    return v


def main():
    repo = os.path.realpath(sys.argv[1])
    ws = load(os.path.join(repo, VENDOR, "Cargo.toml"))["workspace"]
    errors = []
    for copy, (up_dir, allowed, dropped) in CRATES.items():
        here = os.path.join(repo, PATCHED, copy)
        up = os.path.join(repo, VENDOR, up_dir)
        files = {
            os.path.relpath(os.path.join(dp, f), here)
            for dp, _, fs in os.walk(here)
            for f in fs
        }
        if files != allowed:
            errors.append(f"{copy}: holds {sorted(files)}, want {sorted(allowed)}")
        m = load(os.path.join(here, "Cargo.toml"))
        u = load(os.path.join(up, "Cargo.toml"))
        mp, up_pkg = m["package"], u["package"]
        for key in ("name", "version", "license", "edition"):
            if mp.get(key) != field(up_pkg, ws, key):
                errors.append(f"{copy}: {key} is {mp.get(key)!r}, upstream's {field(up_pkg, ws, key)!r}")
        lib = os.path.realpath(os.path.join(here, m.get("lib", {}).get("path", "")))
        if lib != os.path.realpath(os.path.join(up, "src", "lib.rs")):
            errors.append(f"{copy}: [lib] path is {lib}, not the vendored crate's src/lib.rs")
        if "build-dependencies" in m:
            errors.append(f"{copy}: has [build-dependencies]")
        want_build = "build.rs" if copy == "mozjs-sys" else False
        if mp.get("build", "build.rs" if os.path.exists(os.path.join(here, "build.rs")) else None) != want_build:
            errors.append(f"{copy}: build is {mp.get('build')!r}, want {want_build!r}")
        deps, up_deps = m.get("dependencies", {}), u.get("dependencies", {})
        want = {k for k in up_deps if k not in dropped}
        if set(deps) != want:
            errors.append(f"{copy}: dependencies {sorted(deps)}, want {sorted(want)}")
        for k in want & set(deps):
            a, b = deps[k], up_deps[k]
            if isinstance(b, dict) and b.get("workspace"):
                b = {**({"version": ws["dependencies"][k]} if isinstance(ws["dependencies"][k], str) else ws["dependencies"][k]), **{x: y for x, y in b.items() if x != "workspace"}}
            va = a if isinstance(a, str) else a.get("version")
            vb = b if isinstance(b, str) else b.get("version")
            if va != vb:
                errors.append(f"{copy}: {k} version {va!r}, upstream's {vb!r}")
    for e in errors:
        print(f"check_patched_crates: {e}")
    if errors:
        sys.exit(1)
    print(f"check_patched_crates: {len(CRATES)} copies differ from upstream only in their manifests (and mozjs-sys's build.rs)")


if __name__ == "__main__":
    main()
