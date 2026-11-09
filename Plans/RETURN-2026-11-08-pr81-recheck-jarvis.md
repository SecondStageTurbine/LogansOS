<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: relay 360 — #81 re-check (Jarvis)

Dispatch: `Plans/DISPATCH-2026-11-08-relays-359-362.md`, Section 360.
Reviewed head: `5f70ce3459fcfa434bfdf33a1efe99bad3b69168`.
Return branch: `codex/pr81-recheck-jarvis`.

- [x] **PASS — r-efi licence choice.** `third_party/ot61-browser/VENDORED.md:29-33` explicitly records `r-efi` 6.0.0 as taken under MIT OR Apache-2.0, never the LGPL-2.1 option. The upstream expression remains in the provenance table at line 572; the explicit choice resolves the earlier FAIL.
- [x] **PASS — patch/tree reproducibility, all 11 browser patches.** Extracted the browser sources directly from the pinned Git commit into a scratch directory and ran `sh third_party/ot61-browser/verify-patches.sh` there. It exited 0 with 11 `ok` results. Each published pristine `.crate` was downloaded from `static.crates.io`, matched its recorded SHA-256, and accepted its patch; the resulting tree matched `patched/<crate-version>/` under the documented exclusions.

The method is documented in `third_party/ot61-browser/VENDORED.md:36-47` and implemented in `verify-patches.sh:30-46`. Comparison ignores SPDX-only line changes, `.cargo-checksum.json`, and `.cargo-ok`, as the patches do; this is source-tree reproduction under those exclusions, not byte-for-byte reproduction of that metadata.

All patch names below are relative to `third_party/ot61-browser/`. Provenance line numbers refer to that directory's `VENDORED.md` at the reviewed head.

| Patch | Result | VENDORED.md line | Verified upstream .crate SHA-256 |
|---|---|---:|---|
| `libc-0.2.191-neoplutarch.patch` | PASS | 562 | `65cd87c9a6d60536580b316ad985726104730a5151c99845f2490428ef91240a` |
| `parking_lot_core-0.9.12-neoplutarch.patch` | PASS | 571 | `2621685985a2ebf1c516881c026032ac7deafcda1a2c9b7850dc81e3dfcb64c1` |
| `servo-allocator-0.7.0-neoplutarch.patch` | PASS | 578 | `380a2edcbdd9d91d59149f96433a7f1b2d3b128cd0c4055474bca1b76cb22fc1` |
| `servo-base-0.7.0-neoplutarch.patch` | PASS | 579 | `c1bbfbafceeefb49538b928ea7a31c798d15c058ef90d9e2af233f54a4d15fa6` |
| `servo-embedder-traits-0.7.0-neoplutarch.patch` | PASS | 580 | `5f14bbb2593cb2ee9d076b53672703064f30593d6bdb4fab34cd5c0dc6d6da5c` |
| `servo-fonts-0.7.0-neoplutarch.patch` | PASS | 581 | `871ff64ee0d5269a3caeae5c0e758d8e0579987e0187edf86ae0307103359051` |
| `servo-fonts-traits-0.7.0-neoplutarch.patch` | PASS | 582 | `d15da891032b58554795f50b914877cc166eb54665b469cc2df4579307e98d6a` |
| `servo-layout-0.7.0-neoplutarch.patch` | PASS | 583 | `675bf3cb0127b58dde2d072f783c2c57d92c271abd037bc76c781a778af285f4` |
| `servo-paint-api-0.7.0-neoplutarch.patch` | PASS | 584 | `fe5e606d34334b4baccb14d912172e997a3d7dd1c0d19cb2bbd2bb38e5a6c369` |
| `servo-url-0.7.0-neoplutarch.patch` | PASS | 586 | `0b34391568393f40ab8a3b3b7cf41d1fff397ece39e7b0c651a3b513852bb6f5` |
| `stylo-0.22.0-neoplutarch.patch` | PASS | 587 | `7a255e46af2478628c48daab3774090caabd7812b1dd3ca98801c2b9c53491ed` |

This closes the two earlier FAIL items selected by Section 360. The inventory is the 11 browser patches in relay 330's patch/tree finding; unrelated patches elsewhere in the repository and other earlier findings are outside this re-check. No product build or smoke test was required or run.
