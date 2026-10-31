# Kernel Rename Sub-Commit 1 Verification (2026-05-10)

## Scope

Sub-commit 1 implements the kernel-side rename and A3 boot identity from
`Plans/KERNEL-BIBLE-ALIGNMENT-AUDIT-2026-05-10.md`.

## Diff Stat

```text
.cargo/config.toml                                 |  4 ++--
Cargo.lock                                         | 26 +++++++++++-----------
Cargo.toml                                         |  4 ++--
Dockerfile                                         |  2 +-
boot/limine.conf                                   |  6 ++---
docker-compose.yml                                 | 10 ++++-----
kernel/.cargo/config.toml                          |  4 ++--
kernel/Cargo.toml                                  |  2 +-
kernel/linker.ld                                   |  2 +-
kernel/src/drivers/fb/mod.rs                       |  4 ++--
kernel/src/init/boot_frame.rs                      | 13 +++++++----
kernel/src/main.rs                                 |  8 +++----
...x86_64-altimit.json => x86_64-neoplutarch.json} |  0
tests/README.md                                    |  2 +-
tools/xtask/src/main.rs                            | 16 ++++++-------
userland/libneo/Cargo.toml                         |  2 +-
userland/libneo/src/lib.rs                         |  2 +-
userland/neo-init/.cargo/config.toml               |  4 ++--
userland/neo-init/Cargo.toml                       |  2 +-
userland/neo-init/src/main.rs                      |  2 +-
userland/neo-worker/.cargo/config.toml             |  4 ++--
userland/neo-worker/Cargo.toml                     |  2 +-
...imit-user.json => x86_64-neoplutarch-user.json} |  0
23 files changed, 63 insertions(+), 58 deletions(-)
```

## Build Gate

Command:

```text
cargo k
```

Result: PASS.

Kernel warning count: 18, matching the pre-change baseline.

`Cargo.lock` diff is package rename only:
`altimit-neo-kernel` was removed and `neoplutarch-kernel` was added with the
same version and dependency list.

## SMP Integration Gate

Host shell note: direct host `cargo neo-test all` reached xtask but stopped
before QEMU because the Windows shell has no `LIMINE_DIR` and no local
`/home/neo/limine`.

Docker command used for the actual gate:

```text
docker compose run --rm -e NEO_SMP=4 neo cargo neo-test all
```

Result: PASS.

Summary from `target/neo-test/*.serial.log`:

```text
logs=34 pass_logs=33 fail_logs=1
```

The single FAIL log is the deliberate meta-test:

```text
NEO_TEST/1 event=FAIL name=expect_pass_but_fail reason=deliberate_harness_failure
```

The runner reported the deliberate failure was detected.

## Boot Verification

Command:

```text
docker compose run --rm -e NEO_SMP=4 neo bash -lc 'timeout 60s cargo neo-tty'
```

Result: serial boot verified. The command exits via timeout because the
non-test kernel reaches `halt_forever()` after boot, which is expected for
this target.

Serial snapshot:

```text
================================================
  NEOPLUTARCH  v0.0.1
  Intelligence as a kernel primitive.
================================================
...
[ kernel ] framebuffer: 1280x800 @ 32 bpp, pitch 5120 addr 0xffff8000fd000000
[ boot  ] boot sequence rendered (1280x800)
[ kernel ] phase 0 boot complete
[ kernel ] no scheduler yet - halting forever
```

## Rendering Note

`kernel/src/init/boot_frame.rs` does not support smaller typography. The A3
tagline is rendered as a second centered framebuffer line below
`NEO  PLUTARCH` using the same bitmap glyph size and lower-intensity
`Color::TEXT`; the version line remains centered below the tagline in
`Color::NEON_MAGENTA`.

