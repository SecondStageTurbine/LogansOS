<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# third_party/ot61-crt: sources for neo-crt (OT-61 C3a)

`userland/neo-crt` builds the browser bundle's private C/C++ runtime from these
sources. Every directory under `vendor/` is a **pristine** subset of upstream:
whole directories copied from the pinned commit, none edited. Changes live in
`userland/neo-crt/c/` (each derived file names its upstream source and keeps its
licence).

| Source | Licence | Upstream | Pinned commit (date) | Upstream liveness (checked 2026-10-31) |
|---|---|---|---|---|
| musl 1.2.5 | MIT (`vendor/musl/COPYRIGHT`) | git://git.musl-libc.org/musl, fetched from the GitHub mirrors github.com/kraj/musl and github.com/ifduyue/musl, which agree on the tag | `v1.2.5` = `0784374d561435f7c787a555aeab8ede699ed298` (2024-02-29) | active: master `7813e911` 2026-10-14 |
| libc++ 18.1.8 | Apache-2.0 WITH LLVM-exception (`vendor/libcxx/LICENSE.TXT`) | https://github.com/llvm/llvm-project (`libcxx/`) | `llvmorg-18.1.8` = `3b5b5c1ec4a3095ab096dd780e84d7ab81f3d7ff` (2024-06-15) | llvm-project main `29474e1a` 2026-10-30; release 18 itself is closed, so seat D maintains this copy, and the pin moves with the toolchain's clang |

## musl: what is copied

`COPYRIGHT`, `README`, `VERSION`, `include/`, `arch/x86_64/`, `arch/generic/`,
and from `src/`: `include/`, `internal/`, `string/`, `ctype/`, `math/`, `fenv/`,
`stdlib/`, `stdio/`, `multibyte/`, `locale/`, `errno/`, `prng/`, `exit/` (for `assert.c` only; the rest of
`exit/` reaches Linux and isn't compiled), and `time/` (added for C3c-1, from the same tag: `strftime` and its three
date helpers are compiled; the rest of `time/` reaches Linux or reads time zones, and isn't). Directories, not
files, so the copy is easy to re-check against the tag. Which files are
*compiled* is a narrower list in `userland/neo-crt/build.rs`; nothing that makes
a Linux syscall is on it.

## musl: generated headers (`vendor/musl-generated/bits/`)

musl's Makefile generates two headers at build time. They are generated once
with musl's own recipe and checked in, so the build needs no `sed`:

```
sed -f tools/mkalltypes.sed arch/x86_64/bits/alltypes.h.in include/alltypes.h.in > bits/alltypes.h
cp arch/x86_64/bits/syscall.h.in bits/syscall.h
sed -n -e s/__NR_/SYS_/p < arch/x86_64/bits/syscall.h.in >> bits/syscall.h
```

(`bits/syscall.h` is only there because musl's internal `syscall.h` includes it;
no compiled file issues a syscall.)

## libc++: what is copied

`libcxx/include/`, `libcxx/src/`, `libcxx/LICENSE.TXT`, `libcxx/CREDITS.TXT` and
`libcxx/vendor/llvm/default_assertion_handler.in`, from the tag above (a sparse
checkout of those paths). libc++'s CMake would write two configuration headers:
`__config_site` is hand-written in `userland/neo-crt/cxx/__config_site` (the
settings are explained in `docs/THINK-2026-10-31-ot61-c3a-cxx-runtime.md` §3),
and `userland/neo-crt/cxx/__assertion_handler` is the template copied unchanged
(CMake substitutes nothing in it; `build.rs` stops if the copy drifts). Which `src/` files are compiled is
`LIBCXX_SOURCES` in that build script. libc++abi is not used: with exceptions and
RTTI off, the few C++ ABI functions left are in `userland/neo-crt/src/lib.rs`.
