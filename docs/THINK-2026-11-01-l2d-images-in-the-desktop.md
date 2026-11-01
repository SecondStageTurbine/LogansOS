<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: L2d — images in the desktop (the first one, and how it gets there)

**Date:** 2026-11-01 · **Author:** Dexter (Relay 250; Revision 2, Relay 254) · **Status:** design rev 2; no code in this relay.
**Read at:** `origin/main` @ `65b42f3882943d237f44f95e6b9054e19b7c1b4e` (rev 2 rebased; rev 1 read
`42b222cdab393b29b0047077ffae8ed5823b6726`). Every `file:line` below is from the current head
unless it names another commit.
**Inputs:** `docs/THINK-2026-10-31-lived-in-os-lanes.md` §4 (Lane 2 codecs),
`docs/THINK-2026-07-08-gui-vision-cerebro.md`,
`docs/THINK-2026-07-23-desktop-ux-SYNTHESIS.md`, the L2b/L2c RETURNs, and the WM scene spec.

**Bottom line.** The decoder is ready; the *desktop path* is not. `neo-image-decode` is a
host-only std crate on main (`userland/neo-image-decode/src/lib.rs:17-24`), and the WM/Atrium
has no runtime image path at all: its icons are build-time RGBA blobs
(`userland/neo-atrium/src/icons.rs:4-7`) and its only client layers are opaque XRGB surfaces
(`userland/libneo/src/atrium_wm.rs:267-273`). So L2d is not "call the decoder from the WM"; it
is three separate decisions:

1. **App icons do not need runtime decode.** They already ship as pinned blobs; moving a
   decoder into the WM to serve them would break the isolation rule for no visible gain.
2. **A wallpaper is not the first use.** The design authority rejects wallpaper-and-icons as
   the desktop metaphor (`docs/THINK-2026-07-08-gui-vision-cerebro.md:28`,
   `docs/THINK-2026-07-23-desktop-ux-SYNTHESIS.md:14`), and a full-screen client base layer is
   a deeper compositor change than the first image build should carry.
3. **The first use is a one-image viewer client.** It is the smallest thing that exercises
   the whole path — NeoFS read cap → decode in the app's process → G1 surface → WM composite →
   frame hash — and it is the substrate the wallpaper (if ever) and the browser's `<img>` both
   reuse. **Recommendation: build `neo-view` plus a `no_std` PNG backend for
   `neo-image-decode`, with `desktop_image_boot` as the gate.**

## 1. What is already true on main

### 1.1 The decode API, and its one real gap

`userland/neo-image-decode` is the Lane 2b result (`Plans/RETURN-2026-11-01-l2b-image-decode-dexter.md`):

- `decode(bytes)` and `decode_with_limit(bytes, max_rgba_bytes)` (`lib.rs:127-145`) return
  `DecodedImage { width, height, rgba }` (`lib.rs:64-77`), always row-major RGBA8.
- The size cap is checked *before* the pixel allocation: `checked_rgba_len` uses `u64`
  checked multiplication (`lib.rs:155-178`), the PNG `IHDR` is pre-read (`lib.rs:183-191`),
  and JPEG headers are decoded before the scan (`lib.rs:317-333`). The default cap is 64 MiB
  (`lib.rs:41`); callers with a budget use `decode_with_limit`.
- The host KATs and fuzzer cover the hostile-header case (`tools/image-decode-logic-tests`,
  `tools/image-decode-fuzz`; the `u32::MAX` `IHDR` overflow was found in L2c).

**The gap is the OS target.** The implementation is std-only because the vendored `png` 0.18
crate is std-only; `libneo` re-exports the crate behind an off-by-default `image-decode`
feature and refuses it for `target_os = "none"` with a `compile_error!`
(`userland/libneo/src/lib.rs:26-32`, `:62`). That is correct today: a `std` link in a `no_std`
client duplicates `panic_impl` (the L2b RETURN's feature-unification trap). But it means the
desktop cannot use the API until either (a) OT-61's std target is far enough along to run the
existing `png` crate in a userland app, or (b) the crate grows a `no_std + alloc` backend.
This THINK recommends (b), for the first build only (see §9.3).

There is a smaller API problem to fix either way: `ImageDecodeError` exposes the host
decoders' error types directly (`Png(png::DecodingError)` and `Jpeg(zune_jpeg::DecodeErrors)`,
`lib.rs:99-100`). A second backend cannot share that enum. The OS path needs backend-neutral
variants (e.g. `Png(&'static str)`, `Jpeg(&'static str)`) or a `source()` string; the host KATs
should keep the same information. This is a source-compatible change for `Display`, a
breaking change only for callers that match the variants (none outside the crate today).

### 1.2 The compositor seam

The WM is `neo-init` resident as the trusted userland compositor (`userland/neo-init/src/desktop_session.rs`).
It owns the scanout output and a set of WM-owned `Plane`s:

- output, base, and (with motion) `motion_from`/`motion_to` are full-screen
  (`desktop_session.rs:293-341`);
- `client_copy` is the WM's own copy of the client's committed pixels (`desktop_session.rs:297`);
- `Scene::compose` builds `[base, glow, client_copy, busy, lens]` and calls
  `compose_layers_xrgb8888` (`desktop_session.rs:888-1000`). The base is always layer 0 and must
  be opaque and cover the scene (`userland/libneo/src/atrium_wm.rs:311-336`).

The client side is equally narrow: a client creates a G1 surface, imports/attaches a buffer,
and commits (`userland/libneo/src/atrium_protocol.rs:18-39`). The WM maps the client's buffer
read-only and copies it with `SharedPixels::snapshot` (`atrium_wm.rs:204-264`); the copy is
the only thing a `Layer` points at, and the WM never forms a Rust reference over memory a
client can still write (`atrium_wm.rs:193-203`). The buffer format is XRGB8888 only
(`kernel/src/atrium/logic.rs:8-12,118-140`), and `Layer::Opaque` ignores the top byte
(`atrium_wm.rs:267-273`). **There is no alpha in a client surface, no scaling, and no image
object in the protocol.** Any image path must produce opaque, already-sized XRGB pixels.

### 1.3 The capability seam

The WM is the desktop's grant authority. There are two existing ways a file reaches a client:

1. **Spawn-with-cap**: the WM opens a path itself, derives a `USE`-only child cap
   (`userland/neo-init/src/desktop_session/permission.rs:238-250`), and passes that handle to the child
   at spawn (`sys_spawn_budgeted`, `libneo/src/lib.rs:1336-1367`); the child reads it as an
   ordinary file handle. The Lens is a **layout** example, not this recipe:
   `desktop_session.rs:5454-5486` opens its directory with `LOOKUP|READDIR` (`:5462-5466`) and
   uses plain `sys_spawn` (`:5482`); it derives no file `READ` cap and passes no budget. What
   it does show is the fd-slot layout (`:5474-5481`) and the child-reads-its-own-copy pattern
   (`userland/neo-worker/src/lens.rs:35,145-170`). The child gets a handle, never a path
   or a root.
2. **On-demand lend** (the permission pattern): a client sends `OP_PERMISSION` with a path;
   the WM prompts, opens the path itself, derives a `USE`-only child cap, lends it over the
   client's channel, and revokes it at `OP_PERMISSION_DONE`
   (`userland/neo-init/src/desktop_session/permission.rs:9-22,238-250,311-468,470-500`).
   The path must be under `/neofs/`, cannot contain `..`, and cannot name `.system/`
   (`permission.rs:166-179`).

Both are compatible with the rule "decode in the app's process": the WM only *opens and
lends*; it never reads the file's contents into the decode path. The first build should use
(1) for the boot gate and (2) for a user-chosen file later. A hardening note for (1): a plain
`sys_open` under `file-caps` carries `DERIVE|GRANT` meta (`kernel/src/arch/x86_64/syscall.rs:6445-6466`),
so the WM should pass a **derived `USE`-only READ cap**, not its root open, exactly as
`permission.rs:238-250` does. Otherwise a viewer could re-grant the file to another process.

Reads are 4 KiB at a time: `MAX_FILE_IO = 4096` (`kernel/src/arch/x86_64/syscall.rs:1529`), and
the existing small-file helper loops on that (`userland/neo-init/src/desktop_session/login.rs:211-232`).
There is no `seek` and no `stat` on this target (`docs/THINK-2026-11-01-l1d-coreutils-on-logansos.md:25-29,84-86`),
so a reader starts at offset 0 and a re-read needs a fresh open. That matters for the cache key
(§6) and for the encoded-size cap (§4.1).

### 1.4 The memory seam

Three independent ceilings matter:

| Ceiling | Value | Where |
|---|---|---|
| One G1 pixel buffer | 64 MiB | `kernel/src/atrium/logic.rs:20` |
| One `sys_map_region` / heap growth | 16 MiB | `kernel/src/arch/x86_64/syscall.rs:7089` |
| A buffer's width/height | must not exceed the output | `kernel/src/atrium/logic.rs:118-140` |

There is also a real per-process quota: the `mem-quota` kernel feature charges frames to a
`MemAccount` (and `obj-quota` adds kernel-object charges), nests accounts under their spawner,
and lets a parent spawn a child with a frame budget (`kernel/src/mm/mem_account_logic.rs:1-10`;
`kernel/src/atrium/buffer.rs:144-162`; `kernel/src/mm/pmap.rs:484-498,793-840`).
`sys_spawn_budgeted` exposes the frame budget to userland (`userland/libneo/src/lib.rs:1336-1367`),
and `SYS_MEM_INFO` reports the caller's account (`kernel/src/arch/x86_64/syscall.rs:191-194,1103-1104`).

**All of that is conditional on the kernel feature.** The desktop feature set does not enable
`mem-quota` (`kernel/Cargo.toml:118,227-237`), and without it `SYS_SPAWN_BUDGETED` and
`SYS_MEM_INFO` are absent from the dispatch table and return `ENOSYS`
(`kernel/src/arch/x86_64/syscall.rs:1086-1104,1364`). So the WM must call
`sys_spawn_budgeted` on a `mem-quota` kernel and plain `sys_spawn` on the product desktop, and
the product desktop has no per-process frame ceiling today. The image scenario can build with
`mem-quota` and prove the mechanism (§5.3, §7.3); making it a product default is a separate
PM/PL decision. The per-image cap (§5.1) does not depend on the feature at all. When a budget
does exist, the child's image, stack and page tables count against it, so the budget must
include headroom, not just the image bytes.

### 1.5 The design language

The Atrium is deliberately not a wallpaper-and-icons desktop: "It has no wallpaper-and-icons
'desktop,' no taskbar, no Start menu, no window-manager chrome"
(`docs/THINK-2026-07-08-gui-vision-cerebro.md:28`), and "the 'desktop' is the
intent/conversation surface, not a metaphor floor" (`docs/THINK-2026-07-23-desktop-ux-SYNTHESIS.md:14`).
The scene is a Threshold backdrop with two lights and two rings
(`userland/neo-atrium/src/backdrop.rs:44-81`), and its paint order is backdrop → chrome →
client floors (`userland/neo-atrium/src/lib.rs:183-187`). This is a design constraint, not a
technical one, and it reorders the candidate uses (§3).

### 1.6 The gate/oracle seam

The desktop scenarios already have exactly the oracle shape this lane needs:

- the kernel hashes the framebuffer with FNV-1a, masking named label boxes
  (`kernel/src/drivers/fb/mod.rs:475-505`);
- the expected digests are generated independently by `scripts/desktop/scene_oracle.py`, a
  numpy renderer that shares no code with the Rust scene (`scene_oracle.py:1-30`);
- `scripts/desktop/check_fixtures.sh` regenerates every fixture and byte-compares it, and fails
  if a `fixtures/desktop-*` file is not covered (`check_fixtures.sh:11-55`);
- the desktop readback path and its "independent oracle" wording are already in the harness
  (`kernel/src/test_harness.rs:40347-40490`, `:40754`).

For the image gate, the independent side needs an image decode too. The repo already has one:
`tools/image-decode-logic-tests/tests/data/reference_png_rgba.py` is a from-spec PNG decoder
using only Python's `zlib` (`reference_png_rgba.py:1-30`). The oracle should use that, not the
vendored decoder, or the gate degenerates to "the decoder agrees with itself".

## 2. The rule: bytes in the app, pixels through G1

**The rule.** The encoded image is read and decoded **inside the client process that owns the
surface**. The WM and the kernel never parse PNG or JPEG bytes, never allocate a decoded image,
and never hold a decoder. The only thing that crosses into the WM is the client's committed
XRGB pixels, through the same G1 buffer path every other client already uses.

```
 WM (neo-init)                         viewer (neo-view)                    kernel
 ─────────────                         ─────────────────                    ──────
 sys_open(path, READ)
 sys_derive(USE-only READ cap)  ──────► FileHandle (READ)
 sys_spawn_budgeted(view, 64 MiB)   // mem-quota kernel; else plain sys_spawn
                                       read 4 KiB chunks (MAX_FILE_IO)
                                       sniff magic + decode_with_limit
                                       RGBA → opaque XRGB (flatten alpha)
 sys_buffer_map (RO, compositor) ◄───── create buffer + commit
 SharedPixels::snapshot
 compose_layers_xrgb8888
 sys_fb_present                 ─────────────────────────────────────► framebuffer
                                                                          FNV-1a readback
```

Why this is the right boundary:

- **A decoder bug is a client bug.** A hostile PNG exercises `zune-png`/`png` in a process that
  holds one READ cap and no write authority over any file or other process. The WM's copy path
  already assumes a hostile client: it never trusts a `&[u32]` over shared memory and clips
  every client-named rect (`atrium_wm.rs:193-203,244-264,374-391`). The worst a decoder RCE can
  do is corrupt its own process and its own surface; the WM validates the protocol and clips
  the rects.
- **A decode bug in the WM would be a desktop compromise**, because the WM owns scanout, the
  input routes and every client's copy. A decode bug in the kernel would be a system
  compromise. Both are categorically worse than a viewer crash, and both are avoidable at the
  cost of one process boundary.
- **The kernel's job is already right.** It copies file bytes for `sys_read` and pixels for
  `sys_fb_present`; it has no image parser and should not grow one. `MAX_BUFFER_BYTES` and
  `validate_desc` bound the only image-shaped object it sees (`atrium/logic.rs:20,118-140`).

**The proof is mechanical, not a promise.** For the first build, the close gate should assert:

1. `neo-init`'s dependency tree does not contain `neo-image-decode`, `png`, `zune-png` or
   `zune-jpeg` (a `cargo tree -p neo-init` check, plus an `nm` scan of the product binary for
   `png_`/`zune_` symbols).
2. `libneo` still refuses `image-decode` on `target_os = "none"` until the OS backend exists,
   then compiles only the `no_std` backend there (no `std` symbol in the OS binary).
3. The scenario's image bytes are read by the viewer's PID, not the WM's (the viewer logs its
   read count; the WM's log has no read).

If any of these fails, the build is mis-specified regardless of the frame hash.

## 3. Where the desktop would use images, in order

### 3.1 App icons — already build-time, and that is the right shape

The Spine chips draw 40×40 straight-alpha RGBA blobs compiled into `neo-atrium`
(`userland/neo-atrium/src/icons.rs:4-7,29-33`). The masters are SVGs rasterised on the host by
`tools/icon-raster` (`cargo xtask icons`) and pinned by BLAKE3; the scene only blends the
blob (`icons.rs:40-66`; the call sites are `userland/neo-atrium/src/scene.rs:948-975`). There
is no runtime decode, no file read, and no per-boot cost.

Runtime PNG icons would buy exactly one thing: third-party icon packs. The cost is not just a
decoder in the boot path; it is a new **icon-supply protocol** (who supplies the bitmap, when,
at what size, and how it is trusted), or a decoder in the WM (forbidden by §2). The current
pipeline already solves the hard part (deterministic, pinned pixels; the icon box is its own
gate at `kernel/src/test_harness.rs:40460-40490`). **Do not make icons the first image build.**
If third-party icons are ever wanted, decode them at *install* time on the host into the same
`.rgba` blob, or have the app supply a premultiplied bitmap over a new protocol — not a PNG
read in the WM.

### 3.2 Wallpaper — not a lane requirement, and a compositor change

A wallpaper is the obvious "first image" intuition, and it is the wrong first build here for
three independent reasons:

1. **Design.** The Atrium's ground is the Threshold backdrop
   (`neo-atrium/src/backdrop.rs:44-81`), and the design authority explicitly rejects the
   wallpaper-and-icons metaphor (`THINK-2026-07-08-gui-vision-cerebro.md:28`,
   `THINK-2026-07-23-desktop-ux-SYNTHESIS.md:14`). If a wallpaper is ever wanted, it is a
   *content backdrop behind the glass*, not a replacement for the Threshold — a separate
   design decision for the PL, not a technical prerequisite.
2. **Compositor.** The WM's `compose` always uses its own `base` as layer 0 and requires it
   opaque and full-screen (`desktop_session.rs:901-914`; `atrium_wm.rs:311-336`). A
   wallpaper client would either be a new `ROLE_WORKFIELD_ROOT`-style base layer (the role
   exists at `atrium_protocol.rs:154`, and the registry enforces one full-screen root at
   `:834-841`) with the WM's chrome layered above it, or a WM-owned plane. Both change the
   no-stale-pixels argument and the damage model, and both add a trust question (the base is
   the one layer that must cover every pixel).
3. **Memory and scaling.** A full-screen 1920×1080 XRGB plane is 7.91 MiB; a client-owned
   base adds its own buffer plus the WM's copy. There is no scaler in the compositor
   (`atrium_wm.rs:345-369` copies rows 1:1), so a wallpaper at a non-native resolution must be
   scaled by its client, and an 8K image exceeds both the G1 buffer ceiling and the output
   check (`atrium/logic.rs:20,118-140`).

**Verdict:** defer. If it is built later, make it a full-screen *content* surface with a new
base-layer contract, not a WM-side image decode.

### 3.3 The browser's `<img>` path via OT-61 — the real long-term consumer, later

This is the use that justifies a general decoder, and it is the one that should shape the API.
But it is not the first build, for concrete schedule and architecture reasons:

- OT-61 is at C2.4, scoped to the **stylo closure (152 crates)**, while the image/SVG stack
  (`image 0.25.10`, `resvg/usvg`, `png`, `zune-jpeg`, …) is in the **layout closure (828
  crates)** (`docs/RESEARCH-2026-10-30-ot61-stylo-layout-closure.md:17,214`). The `<img>` path
  arrives with the layout closure, not with stylo.
- Servo's image loader and cache live in the browser's process, not the WM. That already
  satisfies §2's process rule; the open question is whether OT-61 uses its own `image` crate
  (and therefore duplicates our decoder) or routes its loader through `neo-image-decode`.
  That is an OT-61 supply-chain/integration decision, not something L2d should pre-empt.
- A browser needs a decoded-image cache with URL keys, multiple sizes, and eviction under
  scroll pressure. A viewer needs none of that. Building the viewer first gives the browser a
  proven decode API and a frame-hash gate to inherit.

**Verdict:** the API should be shaped so the browser *can* use it (backend-neutral errors,
`decode_with_limit`, no global state; §1.1), but the first build should not wait for OT-61.

### 3.4 The one-image viewer — the first use, and the first build

A `neo-view` client that shows one PNG from NeoFS is the smallest end-to-end path:

- it needs no new protocol verb — it is a normal `ROLE_TEXT` client
  (`atrium_protocol.rs:154-156`) using `OP_CREATE_SURFACE`/`OP_IMPORT_ATTACH_BUFFER`/`OP_COMMIT`
  (`:18-20`);
- it needs no WM decoder, no base-layer change and no scaler (the image is sized to the
  client, 640×400, and drawn 1:1);
- it exercises the real capability path and the real compositor, and (on a `mem-quota` kernel)
  the real frame account;
- it produces a frame-hash gate that is independent end to end (Python PNG decode + Python
  scene + kernel readback);
- it is the template for the next two uses: a wallpaper client is a full-screen viewer, and a
  browser `<img>` is a viewer whose source happens to be the network.

The rest of this THINK specifies that build.

## 4. How a PNG reaches the compositor (the first build's path)

### 4.1 The file read, through the capability model

The viewer is spawned with a session channel and one file handle. The WM does the open:

```text
WM: sys_open("/neofs/images/first.png", OPEN_ROOT_TEMPORARY, FileRights::READ)
WM: sys_derive(file, CapRights::new(USE, READ))       // no GRANT on the child
WM: sys_spawn_budgeted("view", [session_client, placeholder, Handle(cap.0)], 64 MiB)
                                                      // mem-quota kernel; else sys_spawn(...)
WM: sys_close(cap); sys_close(file)                   // child holds its own copy
```

The `placeholder` is index 1 because fd 1 is reserved for stdout; the Lens uses the same fd-slot
layout (`userland/neo-init/src/desktop_session.rs:5474-5481`,
`userland/neo-worker/src/lens.rs:29-31`). The spawn-with-cap recipe itself is
`permission.rs:238-250` (derive the `USE`-only cap) plus `libneo/src/lib.rs:1336-1367`
(`sys_spawn_budgeted`); the Lens call site is neither — it opens a directory and uses plain
`sys_spawn` (`desktop_session.rs:5462-5466,5482`). The child reads
with `sys_read` in 4 KiB chunks (`MAX_FILE_IO`, `syscall.rs:1529`), exactly as
`login::read_small_up_to` does (`login.rs:211-232`), until `n == 0`.

Two caps are needed, not one:

- **Encoded bytes**: the API takes a `&[u8]`, so the viewer must materialise the file. A
  16 MiB decoded image can arrive in a 100 MiB file (or a tiny one); cap the encoded read at
  **32 MiB** and stop with a user-visible "too large" rather than allocating the file. Use
  `Vec::try_reserve` so an allocation failure is an error, not an abort.
- **Decoded bytes**: pass the remaining budget to `decode_with_limit` (§5).

The viewer sniffs the magic itself (`ImageFormat::from_magic`, `lib.rs:52-61`) before calling
the decoder, so an unknown file gets a clean message and no decoder call. It does not need a
`stat`: length is learned by reading, and the cap is enforced as it goes.

### 4.2 The decode, in the app's process

The viewer calls the same API the host KATs call:

```rust
let image = neo_image_decode::decode_with_limit(&bytes, 16 * 1024 * 1024)?;
```

The API's guarantees are the ones the desktop needs: dimensions are checked before the pixel
allocation (`lib.rs:155-178,193-229`), the PNG `IHDR` is pre-read (`lib.rs:183-191`), and the
JPEG header is checked before the scan (`lib.rs:317-333`). The first build can be PNG-only in
practice (the gate uses PNG) while the API keeps the JPEG arm available behind the same cap.

The OS-target backend is the one new piece (§9.3): `zune-png` (no_std + alloc, same
`zune-image` repo and triple licence as `zune-jpeg`) replaces the std-only `png` crate on
`target_os = "none"`. The API and the cap logic stay identical; only the `decode_png` body is
target-selected. The host backend stays for the KATs, so the two decoders can be compared
against the Python oracle.

### 4.3 The pixels, through a G1 surface

The client surface is **opaque XRGB8888** (`atrium/logic.rs:8-12,118-140`), so the viewer
must do the alpha flatten itself:

```text
for each pixel:
    if a == 255: out = r<<16 | g<<8 | b
    elif a == 0: out = FLOOR
    else:        out = blend_over(FLOOR, rgba_pixel)
```

`blend_over` is already the WM's straight-alpha formula (`atrium_wm.rs:296-309`), and the
client floor is the scene's content floor `0x0A0E27` (`neo-atrium/src/scene.rs:28`). Reusing
the formula is not a shared mutable object; it is a pure function, and the independent Python
oracle implements the same formula.

There is **no scaling in the compositor** (`atrium_wm.rs:345-369` copies rows 1:1). The first
build therefore requires the image to fit the 640×400 client; a larger image is rejected with
a message ("image is WxH; this viewer shows up to 640x400") rather than silently cropped. A
scaler is a later, separate build; it belongs in the client (the WM must stay image-blind).

The viewer creates its own buffer with `sys_buffer_create`, maps it RW, writes the pixels, and
commits with `OP_COMMIT_DAMAGE` (the full rect) or `OP_COMMIT`
(`atrium_protocol.rs:18-20,39`). This is the Arthur pattern, not the Lens pattern: the client
owns the buffer, and the WM maps it read-only for composition. The alternative (the WM lends a
buffer, as it does for the Lens, `desktop_session.rs:5396-5415`) is also possible; it saves the
viewer a buffer creation but couples the viewer's size to the WM's `client_copy`. For a
first build, the Arthur pattern is the better fit because the viewer chooses its surface size.

### 4.4 The composite, in the WM

The WM copies the committed rect into `client_copy` with `SharedPixels::snapshot`
(`atrium_wm.rs:244-264`; the WM's call site is `desktop_session.rs:868-874`), then
`compose_layers_xrgb8888` writes base + glow + client copy into the output
(`desktop_session.rs:888-1000`). The client copy is `Blend::Opaque`, so the top byte is
discarded and the image is exactly the pixels the viewer wrote (`atrium_wm.rs:267-273`). The
WM never sees the encoded bytes and never calls the decoder.

One WM detail the first build must decide: the scene titles the window "Arthur" and the bar
"Arthur"/"Atrium" (`neo-atrium/src/scene.rs:990,1014-1026`). A viewer called "Image" needs
either a client-supplied name (a small protocol addition) or the first build can keep the
generic title and mask the label boxes in the gate. **Recommendation:** keep the generic title
for build 1; add a `client_name` to `CreateSurface` (or a new `OP_TITLE`) in the same build
that makes the viewer a product feature, not in the gate build. The gate masks labels anyway,
so the image is the only variable.

### 4.5 What each layer never sees

| Layer | Sees | Never sees |
|---|---|---|
| kernel | file bytes as `sys_read` copies; XRGB pixels as a G1 buffer; the framebuffer | PNG/JPEG structure, decoded image objects, the decoder code |
| WM (`neo-init`) | the path at open time; the client's surface size; the committed XRGB pixels | the file's bytes, the decoder, the RGBA buffer, the source image's dimensions |
| viewer (`neo-view`) | the encoded bytes, the decoded RGBA, its own XRGB surface | any other file, write authority, the WM's memory, the framebuffer |
| WM's `client_copy` | one opaque rect of XRGB | alpha, the source image, any metadata |

## 5. Memory budget

### 5.1 Per image

The API returns RGBA8 at `width * height * 4`; the client surface is XRGB8888 at the same
size. The decoder also holds its own intermediate buffer. Concrete numbers:

| Image | RGBA / XRGB each | PNG decode peak (raw + RGBA) | Verdict |
|---|---|---|---|
| 320×200 | 0.24 MiB | ~0.5 MiB | trivially fits |
| 640×400 (the client) | 0.98 MiB | ~2.0 MiB | the first build's size |
| 1280×800 (the resize max) | 3.91 MiB | ~7.8 MiB | fits |
| 1920×1080 | 7.91 MiB | ~15.8 MiB | fits a 1080p screen |
| 2048×2048 | 16.00 MiB | ~32.0 MiB | the proposed per-image cap |
| 3840×2160 | 31.64 MiB | ~63.3 MiB | needs a scaler and a larger budget |
| 4096×4096 | 64.00 MiB | ~128 MiB | over the G1/output limits at 1080p |

The **per-image decoded cap for desktop apps should be 16 MiB** (2048²), not the API's 64 MiB
default. The default is a host-tool convenience; a desktop client that accepted it could
allocate 64 MiB for pixels plus ~64 MiB for the decode buffer, and at 1080p a 4096² surface is
rejected by `validate_desc` anyway (`atrium/logic.rs:118-140`). 16 MiB covers a 1080p image
with 2× headroom and keeps the peak under ~35 MiB. A future 4K/8K viewer passes a larger limit
explicitly and gets a larger budget; it does not change the default.

Two structural limits sit under that:

- **One heap mapping is 16 MiB** (`syscall.rs:7089`). A 16 MiB `Vec` plus allocator metadata is
  split into two contiguous maps by the `libneo` heap; the heap supports up to 64 arenas
  (`userland/libneo/src/lib.rs:2545-2559`), so it works, but a 16 MiB single allocation is at
  the edge. For build 1 (640×400) none of this is close.
- **One G1 buffer is 64 MiB** (`atrium/logic.rs:20`), and its width/height must fit the
  output (`:118-140`). At 1920×1080 a 2048² buffer is already `ExceedsOutput`; the viewer must
  size to the screen/client, not to the source image.

### 5.2 The WM and the viewer, measured

At 1920×1080 the WM's own planes are roughly:

| Plane | Bytes |
|---|---|
| output | 8.29 MiB |
| base | 8.29 MiB |
| `motion_from` + `motion_to` (DM-1) | 16.59 MiB |
| `client_copy` (640×400) | 0.98 MiB |
| glow (~684×470) | 1.23 MiB |
| **WM total** | **~35.4 MiB** |

The viewer adds:

| Item | Bytes |
|---|---|
| G1 surface (640×400) | 0.98 MiB |
| decoded RGBA (640×400) | 0.98 MiB |
| decode peak (raw + RGBA) | ~2.0 MiB |
| code, stack, page tables | a few MiB |
| **viewer total** | **~6–8 MiB** |

So the first build's whole desktop is well under 50 MiB, on a 2 GiB guest (desktop scenarios
get `2048M`, `tools/xtask/src/main.rs:12777`). The budget tables matter for the general case,
not for this image.

**Frame budgets** (enforced only on a `mem-quota` kernel; §5.3):

| Process | Frame budget | Rationale |
|---|---|---|
| viewer, build 1 | **64 MiB** | 16 MiB decoded + ~32 MiB peak + code/stack; the whole 1080p case fits with slack |
| viewer, 4K later | 128 MiB | 31.6 MiB decoded + 63 MiB peak + a scaler |
| browser content process (OT-61) | 128–256 MiB | its own decoded cache; a browser is not one image |
| WM (`neo-init`) | unlimited root, but ~35–40 MiB of planes | the WM is the trusted process; its planes are fixed |

The 64 MiB figure is the **viewer's own account**, not a whole-session ceiling. The WM is the
unlimited root account by design, and its ~35 MiB of planes are neither charged to nor bounded
by the viewer's budget. Nothing in this table caps the sum of the desktop's processes: on a
`mem-quota` kernel each budgeted child is bounded by its own account and by every account above
it, and the unbudgeted root (the WM) is not. On the product desktop (`mem-quota` off) no row in
this table is enforced by the kernel at all.

### 5.3 Enforcement: what is hard, and on which kernel

Revision 1 conflated three separate claims here. They have different enforcement and different
scope:

| Property | Enforced by | Scope |
|---|---|---|
| **Per-image decoded cap** (16 MiB) | viewer: `decode_with_limit` checks `w*h*4` arithmetically before the pixel allocation (`neo-image-decode/src/lib.rs:133-145,155-178`) | **every build**, product included; a larger image is `TooLarge` and nothing is allocated for it |
| **Encoded read cap** (32 MiB) | viewer: stops at the cap, `Vec::try_reserve` (§4.1) | **every build**, product included |
| **Reader process isolation** | kernel process boundary; the WM never decodes (§2) | **every build**; a viewer crash or abort does not take the WM |
| **Viewer frame account** (64 MiB; 16 MiB in the negative leg) | kernel `mem-quota`: `Pmap` frames (`kernel/src/mm/pmap.rs:484-498,793-840`) and G1 buffers (`kernel/src/atrium/buffer.rs:144-162`) are charged before allocation; a refused charge is `ENOMEM` (`syscall.rs:7107-7112`) | **only a `mem-quota` kernel.** The product desktop does not enable it (`kernel/Cargo.toml:118,227-237`) |
| **Whole-session memory ceiling** | nothing in this design | **not claimed.** The WM is unlimited root |

So the honest statement is: **the per-image cap and the process boundary are always enforced;
the per-process account ceiling is enforced when the kernel is built with `mem-quota` — which
the L2d-1 gate scenario does and the product desktop does not.** Until `mem-quota` is a product
default (a separate PM/PL decision: it changes accounting for every process), a product-desktop
viewer is bounded per image (16 MiB decoded, 32 MiB encoded) and contained by its process
boundary, but it has **no hard total-memory ceiling**; a hostile file can still make that
viewer's heap grow until the guest as a whole is under pressure. That is narrower than rev 1's
"an image cannot starve the desktop," and it is the true claim. If a hard product ceiling is
required for L2d-1, enabling `mem-quota` in `desktop` is a prerequisite of the build, not an
optional extra.

**How a refused charge reaches the app.** A charge failure inside `sys_map_region` is `ENOMEM`
(`syscall.rs:7107-7112`), but the path after it matters:
- a direct kernel-object allocation returns it: `sys_buffer_create` maps a refused G1 charge to
  `ENOMEM` (`kernel/src/atrium/buffer.rs:155-161`), and the viewer can report it and keep its
  surface;
- a heap growth goes through the `libneo` allocator, which returns null when its
  `sys_map_region` fails (`userland/libneo/src/lib.rs:2683-2694`). A fallible
  `Vec::try_reserve` then returns an error the viewer prints as `ENOMEM`; an infallible
  allocation — including the vendored decoder's internal buffers — calls the allocator error
  handler and exits 137 (`userland/libneo/src/lib.rs:2729-2735`). The WM survives either way.
  That containment is the property the design needs; "the decoder returns `ENOMEM`" is not
  something a third-party decoder can be assumed to do.

The L2d-1 gate proves both paths: a fallible same-size allocation is refused with `ENOMEM`
under a tight budget (**Q2**), and an infallible decode under a tight budget aborts the viewer
through the 137 path while the WM keeps presenting (**Q3**) — both after the size check has
passed (§7.3).

### 5.4 What the budget does not cover

- **CPU.** A pathological DEFLATE stream can burn a core for a long time. The viewer's process
  isolation keeps the WM live (the WM parks on its channel, not the viewer's decode), but a
  per-decode time budget is a later hardening, not a first-build gate.
- **Product-desktop total memory.** With `mem-quota` off (the product default today) a viewer's
  heap is not capped; the per-image and encoded caps bound one decode, not repeated work or the
  process's live heap. This is the §5.3 gap, stated as a limit rather than hidden.
- **The WM's own planes.** They are fixed and trusted; a hostile client cannot make the WM
  allocate an unbounded plane (the client's `client_copy` is sized by the WM, and the G1
  buffer is charged to the client). This is the property to keep. It is not a whole-session
  ceiling: the WM's planes sit on the unlimited root account and are not charged to any viewer.
- **Kernel memory.** The kernel copies file bytes and maps pages; it does not hold the decoded
  image. `MAX_BUFFER_BYTES` bounds the one image-shaped kernel object.

## 6. Cache and eviction

### 6.1 The surface is the cache (build 1)

Once the viewer has flattened the RGBA into its G1 surface and committed it, the WM has its own
copy in `client_copy`; the viewer can free the RGBA and even the encoded bytes. **Build 1
should cache nothing.** The visible surface is the only retained image, and redraw is a WM
copy, not a re-decode. This is the cheapest correct cache and it makes the first build's
memory story trivial: one surface, one transient decode, then steady state.

### 6.2 An in-app decoded-image cache (later builds)

A cache is needed only when images outlive their surface: thumbnails, a browser's
back/forward, a viewer's next/previous. The design:

| Property | Choice |
|---|---|
| Owner | the app process, never the WM, never a system service |
| Key | path + encoded length + a 64-bit non-cryptographic digest of the encoded bytes (FNV-1a is enough for a cache tag; compare length too) |
| Value | the flattened XRGB surface, or the RGBA if the app will rescale |
| Budget | per app: **32 MiB** for a viewer (≈2 × 1080p), **64 MiB** for a browser content process |
| Unit | decoded bytes, not entry count (a 4K image is not one "slot") |
| Eviction | LRU, with the visible image pinned; never evict the image being composited |
| Admission | an image larger than half the budget is admitted only into an empty cache (otherwise decode-and-draw, do not retain) |
| Invalidation | no `stat` on this target (`THINK-2026-11-01-l1d...:84-86`), so re-open on an explicit user action and replace the entry; a path alone is not a reliable key |
| Teardown | drop non-visible entries on lock, on surface destroy, and on session end |

The key point is that the cache is **per process**. A shared decode service would centralise
the decoder (one bug, every image) and break §2's "decode in the app's process" rule. The
duplication cost (each app has its own decoder and cache) is the price of the isolation, and
it is the same trade browsers make by decoding in their renderer/content processes.

### 6.3 Eviction rules, in order

1. **Pin** the image currently in the composited surface; it cannot be evicted.
2. **Drop** entries on a surface destroy (`OP_DESTROY_SURFACE`, `atrium_protocol.rs:22`) and on
   session end.
3. **Drop non-visible entries on lock.** The WM composes only the lock frame while locked
   (`desktop_session.rs:894-897`), so a cached image is not on screen; dropping it reduces the
   chance that a later unlock shows a stale image and returns memory while the user is away.
   This is a policy, not a confidentiality boundary: the app already holds the file cap.
4. **Evict LRU** until the decoded-byte total is under budget.
5. **Refuse admission** (decode-and-draw) when one entry is over half the budget and the cache
   is not empty.

### 6.4 What we do not build

- **No WM image cache.** The WM's `client_copy` is a copy of one committed surface, not a
  cache of decoded images. It has no decoder and no key space.
- **No kernel image cache.** The kernel has no image objects and should not grow one.
- **No system-wide decode service.** It would be a single high-value target and would make
  every image's pixels cross a process boundary the app does not control.
- **No encoded-bytes cache for local files.** NeoFS reads are cheap relative to a decode; the
  fetch broker may cache HTTP bytes separately, which is a network concern, not this lane.

## 7. The first gate scenario

**Name:** `desktop_image_boot` (kernel feature `neo-test-desktop-image-boot`, user feature
`desktop-image-boot-test`). It needs an entry in both the `TestScenario` table (the shape is
`tools/xtask/src/main.rs:2798-2812`) and `DESKTOP_SCENARIOS` (`tools/xtask/src/main.rs:646`),
which is what gives it the pinned 1920×1080 mode and the 2 GiB desktop memory tier
(`tools/xtask/src/main.rs:12777`).

### 7.1 Boot shape

1. The WM boots with `desktop-image` and `desktop-session-boot-test`, opens
   `/neofs/images/first.png` `READ`, derives a `USE`-only READ cap (`permission.rs:238-250`),
   and spawns `neo-view` with the session channel and the cap — `sys_spawn_budgeted`
   (`libneo/src/lib.rs:1336-1367`) on a `mem-quota` kernel, plain `sys_spawn` otherwise. The
   fd-slot layout is the Lens's (`desktop_session.rs:5474-5481`); the Lens itself derives no
   cap and passes no budget.
2. The viewer reads the file in 4 KiB chunks (encoded cap 32 MiB), sniffs PNG, decodes with a
   16 MiB limit, flattens to XRGB over the floor, creates its 640×400 G1 surface, writes it,
   and commits.
3. The WM copies the committed rect and presents the full scene; the viewer then waits on its
   session channel (like the Lens, `lens.rs:125-134`) so the surface stays up for the readback.
4. The kernel waits for the WM's `DESKTOP image committed` line (the serial script mechanism
   already used by the desktop scenarios), then reads the framebuffer back.

### 7.2 The oracle

The expected frame is computed **independently of the vendored decoder and the Rust scene**:

1. `tools/image-decode-logic-tests/tests/data/reference_png_rgba.py` decodes the same PNG with
   Python's `zlib` and the PNG spec's filters (`reference_png_rgba.py:1-30`).
2. `scripts/desktop/scene_oracle.py` renders the 1920×1080 window view (`render(...)` and
   `digest(...)`, `scene_oracle.py:867-905`); a new `image-fixture <png>` subcommand pastes the
   decoded RGB into the client rect `(frame.x+8, frame.y+34, 640, 400)`, masks the three label
   boxes exactly as the existing fixtures do, and prints the FNV-1a digest.
3. The fixture lands in `fixtures/desktop-image/image_digests.rs`, and
   `scripts/desktop/check_fixtures.sh` gets one `check` line so the
   "every desktop fixture is covered" assertion stays true (`check_fixtures.sh:11-55`).

The kernel compares the full-frame digest with `pixel_digest_masked` (labels masked,
`kernel/src/drivers/fb/mod.rs:475-505`) and also hashes the client rect on its own. The
full-frame digest catches anything the viewer changed outside its surface; the client-rect
digest is the localized image check.

The fixture must record:

- the PNG's SHA-256 and size (the byte-oracle rule: exact bytes from the raw file, not a
  summarizing fetch);
- its decoded RGBA SHA-256 from the Python decoder;
- the scene digest and the client-rect digest, in glass and Reduce-Transparency, focused;
- a **second image** and its digest, as the negative fixture (the gate asserts the actual
  digest equals the first and not the second).

### 7.3 Discriminators (each must be able to fail)

1. **Exact frame digest.** The full-frame FNV-1a equals the independent fixture. If the image
   never reaches the compositor, the digest is the no-image scene's and fails.
2. **Wrong-image fixture.** A different PNG produces a different digest. This proves the gate
   is not a constant.
3. **Channel order.** The oracle's digest hashes all 24 colour bits; a viewer that writes BGRA
   fails (the Python decoder and the kernel hash are both RGB-ordered). No extra control build
   is needed for this: the independent oracle is the control.
4. **Over-cap refusal.** Seed the existing hostile `IHDR` (`u32::MAX` dimensions) or a PNG
   over the 16 MiB decoded cap. The viewer must return `TooLarge`, draw its error floor (or
   exit), and the WM must keep the no-image frame. The gate asserts the log line and the
   unchanged digest; no allocation is attempted (the API pre-check is `lib.rs:155-178,193-229`).
5. **Encoded cap.** A file larger than 32 MiB is refused before the whole read; the viewer
   reports `too large` and does not allocate the file.
6. **Memory quota (`mem-quota` builds only).** Three legs, run in one boot, all through the
   viewer's test-only decode mode (`desktop-image-quota-test`: read → sniff → size check →
   optional preflight → decode, no surface, so the 640×400 fit rule does not intervene). They
   distinguish the *account* check from the *image-size* check; rev 1's single 2048²/32 MiB
   leg could not, because 2048² is exactly at the 16 MiB cap and the decoder's peak was only
   estimated.
   - **Image:** `quota_2000x2000.png`, decoded `2000*2000*4 = 16,000,000` B = 15.26 MiB —
     **within** the 16 MiB cap (`16,777,216` B) with 777,216 B to spare. The cap check is
     arithmetic and uses `<=` (`neo-image-decode/src/lib.rs:155-178`), so `TooLarge` cannot be
     the cause. The image's compressed size is small (a solid or gently graded 2000² PNG), so
     the 32 MiB encoded cap is nowhere near the failure.
   - **Q1, control (64 MiB budget):** the viewer prints `image-size-ok w=2000 h=2000
     rgba=16000000 limit=16777216`, then `image-quota reserve-ok`; it drops the probe, decodes
     and prints `image-quota ok`. Before the reserve it prints `baseline-used=<frames>` from
     `sys_mem_info`. The gate asserts `baseline_bytes + 16,000,000 > 16 MiB`; if that
     inequality fails, the leg cannot prove quota and the gate fails with "test cannot prove
     the quota" (a real discriminator, not a pass).
   - **Q2, account refusal (16 MiB budget):** the same image. The viewer boots, prints
     `image-size-ok`, then `Vec::try_reserve_exact(16_000_000)` is refused because the
     process's own image/stack/page tables are already charged to its account. The viewer
     prints `image-quota ENOMEM` and exits 0 (the marker is the line, not a crash). The gate
     requires, in order: `image-size-ok` → `image-quota ENOMEM`; no `TooLarge`; no
     `reserve-ok`. The size check passed and the only allocation attempted is within the
     decoded cap, so the refusal is the account ceiling.
   - **Q3, infallible decode under a tight budget:** the 2048×2048 PNG from rev 1's leg, a
     16 MiB decoded RGBA (exactly at the cap, so the size check still passes) and a 32 MiB
     budget, decoded in the same test-only mode with no preflight. The decoder's transient
     allocations exceed the account, the allocator returns null, and the infallible path
     aborts: `[ libneo ] allocation failed (OOM); exiting`, exit 137
     (`userland/libneo/src/lib.rs:2729-2735`). The gate asserts that line and the exit, then
     that the WM still presents the previous frame and answers its session channel. This is
     the contained-death path a third-party decoder actually takes, stated as such.
   The WM stays live in all three legs. What is proved is the **viewer's account being charged
   and the refusal staying local**; on the product desktop (`mem-quota` off) Q2 and Q3 do not
   exist, which is exactly why §5.3 does not claim a product session ceiling.
7. **No decoder in the WM.** The dependency/symbol check from §2.
8. **Kill-switch.** `desktop-image` off builds the existing scene byte-for-byte; the
   `desktop_glass_boot` digest is unchanged and the image scenario is not registered.

### 7.4 What the gate does not prove

- **Not a real 4K/8K path.** No scaler, no 4K buffer; the first build is 1:1 at the client
  size.
- **Not a browser.** A browser's `<img>` has URL keys, multiple sizes, and its own cache; those
  are OT-61.
- **Not a security proof of the decoder.** The fuzzer (L2c) and the size cap are the decoder's
  guards; the process boundary is the containment. The gate proves the boundary is where the
  design says it is, not that `zune-png` is bug-free.
- **Not a wallpaper.** A full-screen client base layer is a different compositor contract
  (§3.2).

## 8. What in the WM or Atrium each option touches (from main)

| Option | WM / Atrium touch points | Verdict |
|---|---|---|
| **App icons, runtime** | `neo-atrium/src/icons.rs:4-7,29-33` (build-time blobs; `include_bytes!`), `:40-66` (blend), `neo-atrium/src/scene.rs:948-975` (paint call sites), `tools/icon-raster/src/main.rs` (host rasteriser), the icon-box gate at `kernel/src/test_harness.rs:40460-40490`, `fixtures/desktop-ds1/icon_digests.rs` | **Not first.** A runtime path either puts a decoder in the WM (forbidden) or adds an icon-supply protocol; the build-time path already works and is pinned. |
| **Wallpaper** | `neo-init/src/desktop_session.rs:293-341` (full-screen planes), `:888-1000` (layer order; base is layer 0), `libneo/src/atrium_wm.rs:311-336` (opaque full-scene base required), `:345-369` (1:1 copy, no scaling), `neo-atrium/src/backdrop.rs:44-81` (the Threshold ground it would replace or sit behind), `libneo/src/atrium_protocol.rs:154,834-841` (`ROLE_WORKFIELD_ROOT`, one full-screen root) | **Defer.** A compositor/base-layer change and a design decision, not a first image build. |
| **Browser `<img>`** | `third_party/ot61-browser/` (Servo/vendor); `docs/RESEARCH-2026-10-30-ot61-stylo-layout-closure.md:17,214` (image stack in the 828-crate layout closure, not the 152-crate stylo scope); `neo-image-decode` (the API it could adopt) | **Later.** The real consumer, but gated on OT-61's layout closure and a decode-ownership decision. |
| **One-image viewer (recommended)** | `neo-init/src/desktop_session.rs:5454-5486` (fd-slot layout only), `neo-init/src/desktop_session/permission.rs:238-250` (derived `USE`-only cap), `libneo/src/lib.rs:1336-1367` (`sys_spawn_budgeted`), `:868-1000` (snapshot + compose), `libneo/src/atrium_wm.rs:204-264,315-372` (copy/composite), `libneo/src/atrium_protocol.rs:18-39,154-156` (surface verbs, `ROLE_TEXT`), `neo-worker/src/lens.rs:29-134` (client template), `neo-atrium/src/scene.rs:990,1014-1026` (hardcoded title, a later polish) | **First build.** No new protocol, no WM decoder, a real end-to-end gate. |

## 9. Recommendation: the first build

### 9.1 What it is

**`neo-view` + a `no_std` PNG backend in `neo-image-decode`, gated by
`desktop_image_boot`.** The viewer is a normal `ROLE_TEXT` client; the WM opens and lends a
`USE`-only READ cap and — on a `mem-quota` kernel — spawns it with a 64 MiB frame budget; the
viewer reads, decodes, flattens, commits; the WM composites; the kernel hashes the frame
against the independent oracle. On the product desktop (`mem-quota` off) the same build uses
plain `sys_spawn`, and the claim is the per-image cap plus process containment, not a session
ceiling (§5.3).

It touches:

- `third_party/zune-png` (+ `third_party/zune-inflate` and `simd-adler32` as needed), vendored
  with the same licence discipline as L2b (`VENDORED.md`, `Cargo.toml.upstream`, SPDX lines);
- `userland/neo-image-decode/Cargo.toml` and `src/lib.rs`: target-gated backends, a
  backend-neutral error enum, the same `decode_with_limit` API;
- `userland/libneo/src/lib.rs`: the OS-target re-export (remove/condition the
  `compile_error!` at `:29-32` only for the no_std backend);
- a new `neo-view` client (a `neo-worker` role or a separate crate) using `neo-text` for the
  error floor;
- `userland/neo-init`: a `desktop-image` feature and a spawn branch (test-only at first);
- `kernel/src/test_harness.rs`: the `desktop_image_boot` assertions;
- `tools/xtask/src/main.rs`: the scenario registration;
- `scripts/desktop/image_oracle.py` (or an `image-fixture` subcommand in `scene_oracle.py`),
  `fixtures/desktop-image/image_digests.rs`, and one line in
  `scripts/desktop/check_fixtures.sh`.

### 9.2 Why this and not the alternatives

- **Not icons:** they already work at build time, and a runtime path puts a decoder in the WM
  or invents an icon protocol for no user-visible gain (§3.1).
- **Not wallpaper:** it is not a design requirement, and it needs a base-layer contract and a
  scaler before it is even a clean build (§3.2).
- **Not the browser:** OT-61's `<img>` is behind the 828-crate layout closure and a
  decode-ownership decision; the viewer gives the browser a proven API and gate to inherit
  (§3.3).
- **Not a shared decode service:** one decoder for all images is a single high-value target
  and breaks the process rule (§2, §6.4).

### 9.3 Decoder choice for the OS target

Three options, with the trade:

| Option | Cost | Risk |
|---|---|---|
| Wait for OT-61's std and use the existing `png` crate | couples L2d to OT-61's std timeline; a std binary in the desktop path | the `png` crate's `std::io` path is already fuzzed; no new decoder |
| Vendor `zune-png` (no_std + alloc, same zune-image repo as `zune-jpeg`) | two new zune crates (`zune-png`, `zune-inflate`) plus `simd-adler32` for zlib, and a second inflate implementation | a new decoder to fuzz and KAT against the Python oracle |
| Write a small PNG container on `miniz_oxide` (already vendored) | we own the parser and the filters | a from-scratch decoder is the opposite of the prior-art rule |

**Recommendation: `zune-png`.** It keeps the viewer `no_std`, matches the existing
`zune-jpeg` licence/family, and lets the first build land without OT-61's std. The cost is a
second inflate (`zune-inflate` beside `miniz_oxide`) and a second decoder. Mitigate it with
the existing fuzzer (add a `zune-png` target) and a cross-decoder KAT: the six Lane 2b PNGs
decoded by `png` (host) and `zune-png` (OS backend) must agree with the Python oracle's RGBA
hashes. The zune backend must be selectable in a host test (e.g. a `zune-backend` feature), or
the cross-check can only run on the OS target and never in host CI. If they disagree, the
Python oracle decides which is right; a divergent `zune-png` is a bug to guard or report, not
a reason to weaken the gate.

The API refactor is small but real:

- move the `DecodedImage`/`ImageDecodeError`/`ImageFormat` types and the cap logic into a
  backend-neutral module;
- make the error variants not name the host decoders' types;
- select the `decode_png` body with `#[cfg(target_os = "none")]` (zune) vs the host path
  (`png`), keeping the JPEG arm on `zune-jpeg` for both;
- keep the host KATs and the `libneo::image_decode` re-export path unchanged.

### 9.4 Build order and dependencies

1. **L2d-1 (this build):** the viewer + `no_std` backend + `desktop_image_boot`. Deliverable:
   one PNG from NeoFS on screen, frame-hash green, the WM's dependency tree clean.
2. **L2d-2:** the user-chosen file path: `OP_PERMISSION` read (or a file-manager launch), a
   real title (`client_name`/`OP_TITLE`), and a first cache entry.
3. **L2d-3:** scaling (client-side) and a thumbnail cache; 4K becomes possible.
4. **L2d-4:** the browser's `<img>` integration under OT-61; decide whether Servo's loader
   uses `neo-image-decode` or its own `image` crate.
5. **L2d-5 (only if the PL wants it):** a content backdrop / wallpaper, with a base-layer
   contract.

### 9.5 Falsification tests for the design itself

The design is wrong if any of these fails:

- **F1:** `zune-png` on the six Lane 2b fixtures does not match the Python oracle's RGBA
  hashes. Then the OS decoder is wrong or the fixture is outside its feature set.
- **F2:** the viewer's frame digest equals the no-image scene. Then the image never reached
  the compositor, or the gate is hashing the wrong rect.
- **F3:** a deliberately channel-swapped viewer passes the oracle. Then the oracle is not
  sensitive to colour order (it is: FNV hashes all 24 bits).
- **F4:** an over-cap `IHDR` allocates or panics. Then the size cap is bypassed.
- **F5:** the WM binary contains a `png`/`zune` symbol. Then the decode boundary is violated.
- **F6:** the Q2 leg prints `reserve-ok` or `image-quota ok` under its 16 MiB budget, or
  prints `TooLarge`, or the gate's `baseline_bytes + 16,000,000 > 16 MiB` check fails. Then
  the account is not charged (or the test is mis-sized), and §5.3 is false. Likewise if Q3's
  allocator abort takes the WM down, containment is false.
- **F7:** `desktop-image` off changes the existing scene digest. Then the kill-switch is not
  bit-for-bit and the feature leaked into the base path.

## 10. Prior art

The process rule is not novel; it is the pattern mature systems converged on for untrusted
images:

- **Windows** runs shell thumbnail providers in a separate surrogate host (`prevhost.exe`) so
  a decoder crash or exploit is not `explorer.exe`.
- **macOS** keeps image decoding in ImageIO/QuickLook, out of the window server; the WindowServer
  never parses an image.
- **Browsers** decode images in the renderer/content process, not the browser UI process, and
  cache the decoded result there; the UI process only composites.
- **Linux desktop** historically decoded in-process via GdkPixbuf/ImageMagick shared libraries,
  which is exactly the large shared attack surface this design avoids.

The cost of the pattern is duplicated decoder code and per-process caches. That cost is the
point: the isolation is bought with memory, not with a shared service. For LogansOS, where the
WM owns scanout and the input routes, keeping the decoder out of `neo-init` is the same
trade, one process boundary cheaper than a system service.

## 11. References

- `docs/THINK-2026-10-31-lived-in-os-lanes.md` §4 — Lane 2 (image decoding; host-only guardrails).
- `Plans/RETURN-2026-11-01-l2b-image-decode-dexter.md` — the API, the size cap, the KATs, the
  std-only reason, the feature-unification trap.
- `Plans/RETURN-2026-11-01-l2c-decoder-fuzz-dexter.md` — the fuzzer and the `IHDR` overflow.
- `docs/RESEARCH-2026-10-30-ot61-stylo-layout-closure.md` — why `<img>` is in the 828-crate
  layout closure, not the 152-crate stylo scope.
- `docs/THINK-2026-07-08-gui-vision-cerebro.md` and
  `docs/THINK-2026-07-23-desktop-ux-SYNTHESIS.md` — the no-wallpaper design authority.
- `scripts/desktop/scene_oracle.py`, `scripts/desktop/check_fixtures.sh`,
  `tools/image-decode-logic-tests/tests/data/reference_png_rgba.py` — the independent oracle
  stack the gate reuses.
- `userland/libneo/src/atrium_wm.rs`, `userland/neo-init/src/desktop_session.rs`,
  `userland/libneo/src/atrium_protocol.rs` — the compositor and cap seams cited above.

**Decision requested from the PM/PL:** approve `neo-view` + the `zune-png` OS backend as the
first L2d build, with `desktop_image_boot` as its gate, and keep wallpaper/runtime icons/browser
`<img>` as later, separately scoped builds.

## Revision 2 (Relay 254, answering Chad's #41 review)

Chad's read (`Plans/RETURN-2026-11-01-pr41-review-chad.md`, relay 251) returned three FAILs.
Each is fixed in place above; the changes are:

1. **Per-session enforcement was overstated (FAIL 1).** §5.2 and §5.3 now separate the
   per-image decoded cap and the encoded read cap (enforced in the viewer on *every* build)
   from the per-process frame account (enforced only on a `mem-quota` kernel), and say plainly
   that the 64 MiB is the viewer's own account, not a whole-session ceiling — the WM is
   unlimited root and its planes are not charged to the viewer. The product desktop does not
   enable `mem-quota` (`kernel/Cargo.toml:118,227-237`), so the product claim is narrowed to
   "per-image cap + process containment"; a hard product ceiling is named as an L2d-1
   prerequisite or a later rung, for the PM/PL. §1.4 and §5.4 carry the same correction.
   Also corrected here: without `mem-quota`, `SYS_SPAWN_BUDGETED` is absent and returns
   `ENOSYS` (`syscall.rs:1086-1104,1364`), so the WM must fall back to plain `sys_spawn`; rev 1
   called the budgeted spawn unconditionally.
2. **The quota discriminator did not prove its cause (FAIL 2).** §7.3 item 6 is now three legs
   over a 2000×2000 image whose decoded size (16,000,000 B = 15.26 MiB) is under the 16 MiB
   cap, so `TooLarge` cannot be the cause. Q1 is the control (64 MiB) and prints the measured
   account baseline; the gate checks `baseline + 16,000,000 > 16 MiB` so the negative leg is
   valid. Q2 (16 MiB) passes the size check and is refused by
   `Vec::try_reserve_exact(16_000_000)` with `ENOMEM` — an account refusal, not a size refusal.
   Q3 keeps the rev-1 2048²/32 MiB case but recasts it as what it actually is: the infallible
   decoder's allocator abort (exit 137), contained by the process boundary. F6 is updated to
   match.
3. **The spawn citation was wrong (FAIL 3).** The Lens at `desktop_session.rs:5454-5486` opens
   a directory with `LOOKUP|READDIR` (`:5462-5466`) and uses plain `sys_spawn` (`:5482`); it
   derives no file `READ` cap and passes no budget. All four "spawn-with-cap template" cites
   (§1.3, §4.1, §7.1, §8) now cite `permission.rs:238-250` for the derived `USE`-only cap and
   `libneo/src/lib.rs:1336-1367` for `sys_spawn_budgeted`, and label the Lens only as the
   fd-slot layout example (`:5474-5481`).
4. **Line numbers rebased.** The THINK was rebased from `42b222cda` onto `65b42f388` for this
   revision; `permission.rs` moved by +12 lines, and its citations were re-derived against the
   new head. Every `file:line` above is re-checked against `65b42f388`.
