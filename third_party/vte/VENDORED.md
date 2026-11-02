<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# `vt100` and `vte`: vendored doy/vt100-rust and alacritty/vte

These are the VT parser and screen under the terminal window (Lane 1, rung L1a.2;
`docs/THINK-2026-10-31-l1a-terminal-window.md` §1, §5, §9). `vt100` keeps the screen (cells, attributes, the
cursor, the modes); `vte` is its byte-level parser. Both build with `std` off (`no_std` + `alloc`).

- **Upstream:**
  - `third_party/vt100`: https://github.com/doy/vt100-rust, branch `main`, commit
    `fb0f54ec7222f4805f33cd3ca530229e0a0fe6ad` (2026-10-19). Crate version 0.18.1.
  - `third_party/vte`: https://github.com/alacritty/vte, branch `master`, commit
    `89c9c22f5b8fcc3feba0a9d9f50a020801e74acc` (2026-10-25). Crate version 0.15.0.
  - Both heads were re-checked on 2026-11-01 (unchanged since the THINK's survey on 2026-10-31).
- **Licences:** `vt100` is MIT (`third_party/vt100/LICENSE`, unchanged). `vte` is Apache-2.0 OR MIT
  (`third_party/vte/LICENSE-APACHE` and `LICENSE-MIT`, unchanged); this copy is used under MIT.
- **Manifests:** each `Cargo.toml.upstream` is upstream's `Cargo.toml`, unchanged. Each `Cargo.toml` here differs
  only in `publish = false`, dropped `include`/`exclude`/`keywords`/`categories`/`homepage`/`documentation`/docs.rs
  metadata, `vt100`'s dropped `[dev-dependencies]` (its `tests/` aren't vendored), and `vt100`'s `vte` dependency
  pointing at `../vte` instead of crates.io.
- **Source:** `src/` is upstream's, byte for byte (except the one `vt100` patch below), plus one first line per file (`// SPDX-License-Identifier: MIT`
  in `vt100`, `// SPDX-License-Identifier: Apache-2.0 OR MIT` in `vte`) for LogansOS's SPDX close-gate. Both
  directories are excluded from the LogansOS workspace, so first-party fmt and clippy settings don't apply.
- **rustfmt:** each `rustfmt.toml` is upstream's formatting config plus an SPDX line (`vt100`'s is upstream's
  `.rustfmt.toml`, renamed). `cargo fmt --all` reaches these crates through `neo-term-core`'s path dependency,
  so it must format them by upstream's rules, which leaves the sources unchanged.
- **Not vendored:** `vt100`'s `tests/` (including `tests/data/fixtures`), `examples/`, `bin/`, `fuzz/`; `vte`'s
  `tests/`, `examples/`, `doc/`. The terminal's oracle is the hand-laid one in `userland/neo-term-core`
  (`term.rs`), written from xterm's `ctlseqs.txt`, not vt100's own fixtures.
- **Consumers:** only `userland/neo-term-core` (path dependency, `default-features = false`). **The kernel never
  links them.**
- **Behaviour the terminal relies on, checked by `neo-term-core`'s KATs:** DSR, DA and every other unhandled CSI go
  to `Callbacks::unhandled_csi`; OSC 52 (`copy_to_clipboard`/`paste_from_clipboard`) and title setting are
  callbacks the terminal leaves as no-ops; U+FFFD and C1 code points go to `unhandled_char` and draw nothing; a
  pending wrap is kept as the cursor one past the last column.
- **Maintainer of this copy:** the Lane 1 terminal lane (L1a). Both upstreams are active, so an update is a
  re-vendor at newer commits plus a re-run of `cargo test -p neo-term-core`.

## Checking the base

```
git clone https://github.com/doy/vt100-rust /tmp/vt100
git -C /tmp/vt100 checkout fb0f54ec7222f4805f33cd3ca530229e0a0fe6ad
for f in /tmp/vt100/src/*.rs; do
  tail -n +2 "third_party/vt100/src/$(basename "$f")" | cmp - "$f" || echo "differs: $f"
done
cmp third_party/vt100/Cargo.toml.upstream /tmp/vt100/Cargo.toml

git clone https://github.com/alacritty/vte /tmp/vte
git -C /tmp/vte checkout 89c9c22f5b8fcc3feba0a9d9f50a020801e74acc
for f in /tmp/vte/src/*.rs; do
  tail -n +2 "third_party/vte/src/$(basename "$f")" | cmp - "$f" || echo "differs: $f"
done
cmp third_party/vte/Cargo.toml.upstream /tmp/vte/Cargo.toml
```

**One patch** besides the SPDX lines, in `third_party/vt100/src/perform.rs` (`csi_dispatch`, the `Some(b' ')` arm
for any final but `q`): upstream calls `unhandled_csi` with no intermediates there, so `CSI 6 SP n` looked like DSR
6n to a callback. The patch passes `Some(b' ')` and the second intermediate instead, so the terminal's answerback
filter sees and refuses it (Jarvis's #42 review, item 1; KAT `a_space_intermediate_is_not_dsr` in
`userland/neo-term-core/src/term.rs`). The base check above therefore reports `perform.rs` as differing; the
difference is exactly:

```
-                        None,
-                        None,
+                        Some(b' '),
+                        intermediates.get(1).copied(),
```

`vte` has no patches beyond the one SPDX line per source file.
