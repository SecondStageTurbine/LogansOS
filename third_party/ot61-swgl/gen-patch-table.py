#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Print the [patch.crates-io] table that points a crate at this vendored tree.

Usage: gen-patch-table.py <path from the crate's dir to third_party/ot61-swgl> [Cargo.lock]

With a Cargo.lock (resolved against the full table first), only the packages
in it are listed, so the crate's build prints no "patch was not used" warnings.
Every vendor/<name>-<version>/ becomes one entry (a second version of a name
is keyed <name>_<version>, with `package = <name>`); patched/ and stubs/ replace
the vendor copy of the same crate. Paste the output below a crate's
[dependencies]; xtask's closure build fails if anything resolves to crates.io.
"""
import os
import re
import sys

rel = sys.argv[1].rstrip("/")
keep = None
if len(sys.argv) > 2:
    import tomllib

    with open(sys.argv[2], "rb") as f:
        keep = {(p["name"], p["version"]) for p in tomllib.load(f)["package"]}
here = os.path.dirname(os.path.abspath(__file__))
crates = {}
for kind in ("vendor", "patched", "stubs"):
    d = os.path.join(here, kind)
    if not os.path.isdir(d):
        continue
    for n in sorted(os.listdir(d)):
        m = re.match(r"(.+?)-(\d+\.\d+\.\d+.*)$", n)
        if m is None or not os.path.isdir(os.path.join(d, n)):
            continue  # not a <name>-<version> crate directory (e.g. .gitattributes)
        if keep is None or (m.group(1), m.group(2)) in keep:
            crates[(m.group(1), m.group(2))] = f"{rel}/{kind}/{n}"
print("[patch.crates-io]")
seen = {}
for (name, ver), path in sorted(crates.items()):
    seen.setdefault(name, 0)
    if seen[name] == 0:
        print(f'{name} = {{ path = "{path}" }}')
    else:
        key = name + "_" + re.sub(r"[^0-9A-Za-z]", "_", ver)
        print(f'{key} = {{ package = "{name}", path = "{path}" }}')
    seen[name] += 1
