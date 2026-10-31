<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family adversarial review of Desktop S1 (resident session WM + live input) → a Codex seat

**Type:** review only (no code to merge). **Deliverable:** a pushed RETURN file. **Why a Codex seat:** CLAUDE.md
requires a different-model-family lens on merges that touch the cross-CPU / input-routing / capability seams. The
author and the five AAG lenses on this branch are all Claude-family, so their agreement is weak evidence. **Merge
does not wait on this review** (PL rule: merge on gate + Claude AAG, Codex batched); a surviving finding becomes a
fix-forward or a revert per the escaped-defect contract. Seat: the PL picks (Jarvis / Janus / Chad).

## What is under review

PR #25, branch `claude/desktop-s1-session-boot`. The PM names the exact head SHA in the PR comment that relays this
dispatch; review the whole diff at that head (`git diff origin/main...origin/claude/desktop-s1-session-boot`).

Desktop S1 turns a non-test boot into a live, full-screen desktop:

- **Kernel features.** `desktop-session` (default-off; = `atrium-input-routing` + `usb-hid-mouse`) and the composed
  `desktop` set (`product` + `desktop-session` + `kernel-sysstat`, `phase9-real-quant`, `threaded-matmul`,
  `phase9-avx2-matmul`, `forward-perf`) in `kernel/Cargo.toml`.
- **Runtime switch.** `ATRIUM_RUNTIME_ON` is now `cfg!(any(neo-test-atrium-runtime, desktop-session))`
  (`kernel/src/atrium/mod.rs`).
- **Production boot path** (`kernel/src/main.rs`, block `cfg(all(feature = "desktop-session", not(feature =
  "neo-test")))`). The framebuffer is initialized before neo-init, the Software present backend is selected, and
  `set_gpu_submit_enforce(true)` arms the scanout gate. `KERNEL_READY` deliberately stays unset while the session
  runs. neo-init then stays resident; if it ever returns, the old splash is the fallback.
- **GpuSubmit cap handoff.** neo-init inherits the root `GpuSubmit` cap under `desktop-session` (the compositional
  root-cap list in `spawn_neo_init`, CP-34 shape). Arthur is spawned by neo-init with only the session channel,
  two fresh channel ends, and an optional LOOKUP|READDIR memory directory (`spawn_arthur`).
- **New ABI.** `SYS_FB_OUTPUT_QUERY = 0x1B0` (`kernel/src/arch/x86_64/syscall.rs`, `sys_fb_output_query_impl`)
  returns `pack_output_size(w, h)` (`kernel/src/atrium/logic.rs`: width bits 63..32, height bits 31..0, both
  clamped to 31 bits) to a caller that holds GpuSubmit. Otherwise it returns EPERM, or ENOSYS when the runtime
  switch is off. libneo inverse: `sys_fb_output_query` / `unpack_output_size`. PM-ratified 2026-10-18.
- **The WM** (`userland/neo-init/src/desktop_session.rs`). It owns a full-screen output buffer plus a base and a
  glow plane. It composes base + focus light + Arthur's RO-mapped client buffer via `compose_layers_xrgb8888`
  (`userland/libneo/src/atrium_wm.rs`) for the damage rect, commits that rect, presents at (0,0), and waits on the
  present fence before replying to Arthur's commit. Layout and paint come from `userland/libneo/src/atrium_desktop.rs`.
- **Arthur** (`userland/neo-arthur/src/main.rs`, `run_desktop_session`; client surface unchanged in
  `showcase_surface.rs`). It busy-polls `sys_input_focus_recv` (EAGAIN → `spin_loop`). That is a **known, accepted
  limitation**: a bounded deadline focus-recv is a separate rung. It tolerates a missing model.
- **Live input.** qemu-xhci + usb-kbd + usb-mouse. The HID drivers drain from syscall context
  (`pump_routed_events` inside `sys_input_focus_recv_impl`) into `focus::route_event`. The scenario build has no
  test-inject syscall.
- **Discriminator.** Scenario `desktop_session_boot` (`tools/xtask/src/main.rs`; `desktop_session_asserts` in
  `kernel/src/test_harness.rs`):
  - boots at the pinned 1920x1080 and types `hello desktop` + Enter via HMP `sendkey` on the usb-kbd;
  - requires Arthur's ordered serial echo;
  - after the session ends, FNV-digests every framebuffer pixel against `atrium_desktop::scene_pixel`;
  - asserts HID `reports>0, legacy_bytes=0, ep_errors=0`.
- **Host KATs** are `s1_*` in `tools/atrium-logic-tests/src/lib.rs`: hand-laid blend, compose, layout,
  scene-pixel, and register-encoding oracles.

## Your job: refute it

Default to suspicious. Specifically try to break:

1. **The ADR-074 input-routing seam under SMP.**
   - Arthur's tight `sys_input_focus_recv` loop runs on one CPU. On another CPU, any process's syscall can run
     `pump_routed_events` → `drain` → `route_event`. `route_event` uses `DELIVERY.try_lock()` and the drivers
     ignore its boolean.
   - Can a keystroke (press or release) be lost, duplicated or reordered? Can a lost release leave a key stuck?
   - Is there any path where the pump runs with interrupts off while holding `USB_DEVICE`-ordered locks and
     contends with the xHCI MSI ISR (`EVENT`/`PENDING`/`EP_ROUTES`)?
   - Is the focus ACTIVATE/RECEIVE split still airtight now that it runs outside a scenario? Can the WM's
     activate-only handle ever observe an event?
2. **The shared-pixel path.**
   - Arthur writes its RW mapping; the WM reads an RO mapping of the same frames as a `&[u32]` while composing.
     The protocol orders writes before commit and reads after, under the channel rendezvous.
   - What happens if Arthur, malicious or buggy, writes during the WM's compose? Torn pixels only, or anything
     worse (UB the compiler can exploit, a fault, a panic in `compose_layers_xrgb8888`)?
   - What happens if Arthur exits, or destroys the surface, while the WM still holds the mapping? Check the
     `Arc` pinning in `sys_buffer_map_impl` and the borrowed Pmap leaves.
3. **The GpuSubmit cap handoff.**
   - Prove or refute that only neo-init can reach raw scanout in a production `desktop` boot: Arthur, a
     sys_spawn child of Arthur, or any process holding a derived or granted handle.
   - Is `set_gpu_submit_enforce(true)` armed before any user code can run on any CPU? APs are already up at that
     point.
   - Can `sys_grant` or `sys_derive` move or attenuate a `GpuSubmit` handle out of neo-init?
   - Does the CP-34 root-cap list still produce the right handle set for every feature combination that includes
     `desktop-session` (with and without `net-dial-cap`, `browser-runtime`)?
4. **`SYS_FB_OUTPUT_QUERY`.**
   - Check the number against the whole syscall table and libneo.
   - Check sign and width of the packed return. Can any `(w, h)` produce a value that reads as an errno, or lose
     bits?
   - Is the authority check identical to `sys_fb_present_impl`'s, including the rights bits `holds_gpu_submit`
     requires? Is the information it leaks to the cap holder acceptable?
   - Is the ENOSYS / EPERM / EINVAL truth table right when the framebuffer is absent?
5. **The production boot path vs the scenario.**
   - The scenario runs the neo-test boot order. Production calls `initialize_framebuffer(true)` before neo-init and
     again on fallback, and runs `boot_eth0::bring_up()` on the same xHCI as the HID devices.
   - What breaks in production that the gate cannot see?
6. **Kill-switch.**
   - With `desktop-session` off, is every build bit-for-bit (or `.text`-identical) to `main`? The author measured
     the default kernel's `.text` as byte-identical; the only `.rodata`/`.data` deltas were the git SHA and
     panic-location line numbers.
   - Check the cfg edits in neo-arthur and neo-init too: `not(desktop-session)` additions, the dead-code allow
     lists, and the `run_selected_scenario` exclusions.
7. **Discriminator validity.**
   - Can `desktop_session_boot` pass vacuously? Consider: the oracle module is mounted from the same
     `atrium_desktop.rs` the WM paints with, and the readback happens after focus is withdrawn.
   - Do the hand-laid `s1_scene_pixels_hand_laid_1920x1080` values break the closed loop?
   - Would a WM that never presented, a stale BOOT indicator, or a mis-routed keystroke each fail it?
8. **Anything else** a Claude reviewer would structurally miss: concurrency pitfalls in
   `docs/concurrency-pitfalls.md` (CP-14/15/16/17/34/37 are the closest), I-14 isolation, and the u16 damage-rect
   bound at outputs over 65535 px.

A finding bounces the branch only if it survives your own re-verification; uncertain items go in the RETURN as
notes. "No findings" must state what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\<your seat>     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/desktop-s1-crosscheck-<seat>       (git fetch origin && git checkout -B codex/desktop-s1-crosscheck-<seat> origin/claude/desktop-s1-session-boot)
```

Read this dispatch and the code from the branch. If the PR has merged by the time you start, read them from `main`
instead.

```
git fetch origin
git show origin/claude/desktop-s1-session-boot:Plans/DISPATCH-2026-10-18-desktop-s1-codex-crosscheck.md
git diff origin/main...origin/claude/desktop-s1-session-boot
```

Building and running are optional.

- `cargo xtask neo-test desktop_session_boot` should PASS in about 30 s.
- To see the live desktop: `cargo xtask run-desktop` (or `--vnc`). The plain-language guide is
  `docs/HOWTO-run-desktop.md`.
- Negative controls:
  - Drop the WM's first full-screen present (`scene.present(None, layout.screen)` in `desktop_session.rs`) and
    have the WM present only Arthur's rect. The readback must FAIL with `desktop_scene_not_full_screen_wm_owned`.
  - Change `desktop_input_devices()` to omit `usb-kbd`. The serial script must stall at step 2 and the scenario
    FAIL.

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?"
questions (faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='input focus route_event pump_routed_events DELIVERY'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-18-desktop-s1-codex-crosscheck-<seat>.md` to `codex/desktop-s1-crosscheck-<seat>` and
**push it**. A review delivered only in chat does not exist.

The RETURN contains:
- the verdict: MERGE / MERGE-WITH-NITS / BOUNCE. If #25 has already merged: NO-ACTION / FIX-FORWARD / REVERT;
- each finding with severity, file:line, and a repro or reasoning;
- for each numbered item above, what you checked.

Paste the pushed commit SHA in the reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree afterwards. Never
delete the worktree itself.
