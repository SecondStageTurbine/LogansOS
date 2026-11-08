#!/usr/bin/env python3
# SPDX-License-Identifier: MPL-2.0
"""Print VENDORED.md's crate table for this tree, one row per crate.

Usage: gen-vendored-table.py

Every value comes from the crate's own files, never typed by hand:
- name, version, licence and repository from its Cargo.toml;
- the upstream commit from .cargo_vcs_info.json (the commit the release was
  published from; a crate published without one gets a footnote mark);
- the crates.io .crate SHA-256 from .cargo-checksum.json's "package".
A patched crate (patched/) has no checksum file of its own, so its published
.crate SHA-256 is recorded in PATCHED_CRATE_SHA256 below, from the copy it was
made from. A stub (stubs/) is not the published crate and has no SHA.
"""
import json
import os
import tomllib

PATCHED_CRATE_SHA256 = {
    "parking_lot_core-0.9.12": "2621685985a2ebf1c516881c026032ac7deafcda1a2c9b7850dc81e3dfcb64c1",
    "stylo-0.22.0": "7a255e46af2478628c48daab3774090caabd7812b1dd3ca98801c2b9c53491ed",
    # OT-61 L0 (#55): sha256sum of the downloaded .crate, equal to the
    # registry index's "cksum" for the release (checked for servo-layout and
    # libc).
    "libc-0.2.191": "65cd87c9a6d60536580b316ad985726104730a5151c99845f2490428ef91240a",
    "servo-allocator-0.7.0": "380a2edcbdd9d91d59149f96433a7f1b2d3b128cd0c4055474bca1b76cb22fc1",
    "servo-base-0.7.0": "c1bbfbafceeefb49538b928ea7a31c798d15c058ef90d9e2af233f54a4d15fa6",
    "servo-embedder-traits-0.7.0": "5f14bbb2593cb2ee9d076b53672703064f30593d6bdb4fab34cd5c0dc6d6da5c",
    "servo-fonts-0.7.0": "871ff64ee0d5269a3caeae5c0e758d8e0579987e0187edf86ae0307103359051",
    "servo-fonts-traits-0.7.0": "d15da891032b58554795f50b914877cc166eb54665b469cc2df4579307e98d6a",
    "servo-layout-0.7.0": "675bf3cb0127b58dde2d072f783c2c57d92c271abd037bc76c781a778af285f4",
    "servo-paint-api-0.7.0": "fe5e606d34334b4baccb14d912172e997a3d7dd1c0d19cb2bbd2bb38e5a6c369",
    "servo-url-0.7.0": "0b34391568393f40ab8a3b3b7cf41d1fff397ece39e7b0c651a3b513852bb6f5",
}

here = os.path.dirname(os.path.abspath(__file__))


def read_json(path):
    try:
        with open(path) as f:
            return json.load(f)
    except FileNotFoundError:
        return None


def repo(url):
    url = (url or "").strip()
    for prefix in ("https://", "http://"):
        if url.startswith(prefix):
            url = url[len(prefix):]
    url = url.rstrip("/")
    if url.endswith(".git"):
        url = url[: -len(".git")]
    return url or "—"


rows, notes = [], []
for kind in ("vendor", "patched", "stubs"):
    d = os.path.join(here, kind)
    if not os.path.isdir(d):
        continue
    for entry in sorted(os.listdir(d)):
        path = os.path.join(d, entry)
        if not os.path.isdir(path):
            continue
        with open(os.path.join(path, "Cargo.toml"), "rb") as f:
            pkg = tomllib.load(f)["package"]
        name, version = pkg["name"], pkg["version"]
        vcs = read_json(os.path.join(path, ".cargo_vcs_info.json"))
        commit = f"`{vcs['git']['sha1']}`" if vcs and "git" in vcs else "not published¹"
        if commit == "not published¹" and kind != "stubs":
            notes.append(f"{name} {version}")
        if kind == "vendor":
            sha = f"`{read_json(os.path.join(path, '.cargo-checksum.json'))['package']}`"
            label = name
        elif kind == "patched":
            sha = f"`{PATCHED_CRATE_SHA256[entry]}`"
            label = f"{name} (patched)"
        else:
            sha = "— (a stub, not the published crate)"
            label = f"{name} (stub)"
            commit = "— (stands in for this repository's crate; no code)"
        rows.append(
            (kind != "vendor", name.lower(), version,
             f"| {label} | {version} | {pkg.get('license', '—')} | {repo(pkg.get('repository'))} | {commit} | {sha} |")
        )

print("| Crate | Version | Licence | Repository | Upstream commit | crates.io `.crate` SHA-256 |")
print("|---|---|---|---|---|---|")
for *_, line in sorted(rows):
    print(line)
if notes:
    print()
    print(f"¹ Published without `.cargo_vcs_info.json` ({', '.join(notes)}); "
          "its integrity is the crates.io checksum.")
