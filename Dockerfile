# NeoPlutarch — development container.
#
# This image bundles everything needed to build and run the NEO kernel:
#   - Rust nightly (pinned via rust-toolchain.toml at runtime)
#   - rust-src + llvm-tools for build-std
#   - QEMU x86_64 and aarch64 system emulators
#   - xorriso + the Limine bootloader (for Phase 0.1 ISO assembly)
#   - A non-root `neo` user so files we create on the mounted volume
#     don't end up owned by root on the host.
#
# Image stays single-stage; build artifacts live on the host volume,
# not in the image, so there's nothing to optimize away.

FROM debian:trixie-slim@sha256:b6e2a152f22a40ff69d92cb397223c906017e1391a73c952b588e51af8883bf8

ARG DEBIAN_FRONTEND=noninteractive

# System packages: build tools, packaged QEMU utilities, ISO tools, basic utilities.
# We keep this list intentionally tight — anything else can be added
# later without a rebuild via `apt install` at runtime if needed.
RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        curl \
        e2fsprogs \
        git \
        gnupg \
        libfdt-dev \
        libglib2.0-dev \
        libpixman-1-dev \
        libslirp-dev \
        nasm \
        ninja-build \
        pkg-config \
        python3 \
        python3-venv \
        qemu-system-x86 \
        qemu-system-arm \
        qemu-utils \
        xorriso \
        mtools \
        sudo \
    && rm -rf /var/lib/apt/lists/*

# Debian trixie's packaged QEMU 10.0.8 exposes but cannot instantiate the
# generic vhost-user development device. Build a pinned QEMU 10.x release that
# includes the user-creatable vhost-user-test-device-pci model.
ARG QEMU_VERSION=v10.2.3
RUN git clone --depth 1 --branch ${QEMU_VERSION} \
        https://gitlab.com/qemu-project/qemu.git \
        /tmp/qemu \
    && cd /tmp/qemu \
    && ./configure \
        --prefix=/usr/local \
        --target-list=x86_64-softmmu,aarch64-softmmu \
        --disable-docs \
        --disable-gtk \
        --disable-sdl \
        --disable-opengl \
    && make -j"$(nproc)" \
    && make install \
    && rm -rf /tmp/qemu

# OVMF, pinned (OT-87 R1, `cargo xtask vm-image` / `vm_installed_two_boot`):
# the edk2 build QEMU v10.2.3 ships in pc-bios/ (edk2-stable202408.01, commit
# 4dfdca63a93497203f197ec98ba20e2327e4afe4), which `make install` unpacks into
# /usr/local/share/qemu (its configure requires bzip2 to install blobs). No
# separate download and no host package: the bytes are the QEMU tag's. xtask
# holds the same two files to BLAKE3 pins (tools/xtask/src/vm_image.rs).
RUN cd /usr/local/share/qemu \
    && echo "33090cc07675baa5190d9f1e84bf5176b33bcbfa9bacac522961150cdb6dbb2a  edk2-x86_64-code.fd" | sha256sum -c - \
    && echo "5d2ac383371b408398accee7ec27c8c09ea5b74a0de0ceea6513388b15be5d1e  edk2-i386-vars.fd" | sha256sum -c -

# Limine: clone the binary release branch. Phase 0.1's ISO builder
# needs the limine binary to install BIOS/EFI boot code on disk images.
# We pin to a specific tag so reproducible builds stay reproducible.
ARG LIMINE_VERSION=v8.6.0-binary
RUN git clone --depth 1 --branch ${LIMINE_VERSION} \
        https://github.com/limine-bootloader/limine.git \
        /opt/limine \
    && make -C /opt/limine \
    && chmod -R a+rX /opt/limine

ENV LIMINE_DIR=/opt/limine

# OT-61 C3a: neo-crt (musl + libc++) and SWGL's C++ compile with clang, so
# scenarios that link them (cxx_runtime_smoke, the SWGL/WebRender smokes)
# need it in the gate. Debian trixie's packaged LLVM, held to major 19: the
# image build fails if apt resolves another major, so a mirror change can't
# move the gate's compiler silently. Its own layer, after the QEMU and
# Limine builds, so their cached layers stay valid.
ARG NEO_CRT_CLANG_MAJOR=19
RUN apt-get update && apt-get install -y --no-install-recommends \
        clang-${NEO_CRT_CLANG_MAJOR} \
    && rm -rf /var/lib/apt/lists/* \
    && clang-${NEO_CRT_CLANG_MAJOR} --version \
    && test "$(clang-${NEO_CRT_CLANG_MAJOR} -dumpversion | cut -d. -f1)" = "${NEO_CRT_CLANG_MAJOR}"
ENV NEO_CRT_CLANG=clang-${NEO_CRT_CLANG_MAJOR}

# Create the neo user. UID/GID 1000 lines up with the typical first
# user on Linux hosts; on Windows/Mac with Docker Desktop the file
# ownership translation is handled by the Docker VM.
RUN useradd --create-home --shell /bin/bash --uid 1000 neo \
    && echo "neo ALL=(ALL) NOPASSWD:ALL" > /etc/sudoers.d/neo

USER neo
WORKDIR /home/neo

# Install rustup + a default nightly. The exact channel will be
# overridden by the project's rust-toolchain.toml on first build,
# but we pre-fetch a recent nightly so the first run isn't a
# 5-minute download.
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
        | sh -s -- -y --default-toolchain nightly \
                      --profile minimal \
                      --component rust-src,llvm-tools-preview,clippy \
    && /home/neo/.cargo/bin/rustup target add x86_64-unknown-none aarch64-unknown-none

ENV PATH=/home/neo/.cargo/bin:/usr/sbin:/sbin:$PATH
ENV CARGO_TERM_COLOR=always

# The repo gets mounted at /workspace at runtime.
WORKDIR /workspace

# The bind-mounted /workspace is often owned by another uid (0:0 on a Windows
# Docker host), and git refuses such a tree as "dubious ownership", so every
# `git rev-parse HEAD` failed and every docker-built kernel embedded
# `unknown-build-id` (A7, A8). Trust the mount for every git in the
# container, without writing a config file (docker-compose.yml sets the
# same, so an existing image needs no rebuild).
ENV GIT_CONFIG_COUNT=1 \
    GIT_CONFIG_KEY_0=safe.directory \
    GIT_CONFIG_VALUE_0=/workspace

# Default command drops you into a shell. Override with `cargo neo`
# or whatever you want to run.
CMD ["bash"]
