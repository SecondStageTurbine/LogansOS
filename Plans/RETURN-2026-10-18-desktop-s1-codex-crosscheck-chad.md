<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Desktop S1 cross-family adversarial review (Chad)

**Verdict:** FIX-FORWARD (PR #25 is merged as `0f94beec`).

**Reviewed:** the merged S1 change and its current descendants on `origin/main` at `1a840e54`; focused reads of the original merge commit, current input routing, buffer mapping/lifetime, capability derivation/grant, production boot, syscall ABI, WM/client protocol, and discriminators. The semantic index was healthy (`ollama=true`, `qdrant=true`) and was used for discovery; exact symbols and final evidence were checked in the source.

## Findings

### 1. HIGH — SMP lock contention silently consumes HID reports before they reach any focus queue

**Evidence:** `kernel/src/atrium/focus.rs:102-110,131-145`; `kernel/src/drivers/usb/hid_keyboard.rs:179-197,226-230`; `kernel/src/arch/x86_64/syscall.rs:6369-6409`.

`InputFocus::try_recv` takes `DELIVERY.lock()`. On another CPU, `sys_input_focus_recv_impl` may simultaneously pump a completed keyboard report. `Inner::drain` advances the keyboard decoder, calls `route_event`, ignores its boolean result, reposts the USB slot, and continues. `route_event` uses `DELIVERY.try_lock()` and returns `false` on contention. The completed report has therefore been consumed with no retry or pending handoff.

This can lose either a key press or its release. In particular, after `BootKeyEventDecoder` has observed and emitted a release once, a later identical report does not recreate it, so a client which tracks key state can remain stuck. Survivors retain order and are not duplicated, but the stream is not reliable. The newer class-aware queue policy (`13287c58`) protects keys only after `route_event` acquires `DELIVERY`; it does not cover this pre-queue drop.

**Repro/reasoning schedule:** CPU 0 enters the focused client's `sys_input_focus_recv`, reaches `focus.try_recv()`, and holds `DELIVERY`. CPU 1 enters any input-focus receive syscall and pumps a just-completed release report. Its keyboard lock serializes the device drain, but `route_event` fails `try_lock`; the driver ignores `false` and reposts the slot. The release is gone. An SMP test should deliberately hold/instrument `DELIVERY` while draining a press/release pair and require eventual ordered delivery of both.

**Fix direction:** do not make lock contention an unobservable input loss. A per-device/per-CPU bounded staging queue, or a retryable pending report/event which is not acknowledged/reposted until enqueued, would preserve the non-blocking ISR constraint. Counted overflow policy should remain distinct from lock-contention loss.

### 2. MEDIUM — the compositor gives asynchronously writable shared pages a long-lived ordinary Rust slice

**Evidence:** `userland/neo-init/src/desktop_session.rs:491-497` (current merged copy; the imported mapping is converted with `from_raw_parts` and retained); `userland/libneo/src/atrium_wm.rs:202-297`; `kernel/src/arch/x86_64/syscall.rs:5775-5822`.

The protocol orders the well-behaved Arthur writer before COMMIT and the WM read after receiving COMMIT, but the kernel does not revoke or freeze Arthur's RW mapping while the WM composes. A malicious or buggy Arthur can write the same physical pages while neo-init holds and reads an `&[u32]`. Hardware effects are bounded to inconsistent/torn visual data (the descriptor and indices are kernel-owned and validated, so the composition loops do not gain an out-of-bounds index from pixel contents), and the mapping pin prevents use-after-free. However, `slice::from_raw_parts`' ordinary shared-reference contract requires the referenced memory not be mutated for the reference lifetime outside `UnsafeCell`; the cross-process RW alias does not satisfy that contract. Treating the pages as a stable `&[u32]` is therefore not a sound Rust abstraction for an adversarial client, and compiler reuse/hoisting cannot be dismissed merely because the protocol says “commit first.”

**Fix direction:** snapshot through volatile/atomic loads into WM-owned storage inside a protocol-defined read window, or add a kernel-enforced ownership/freeze/copy transition before exposing an ordinary slice. Keeping the physical mapping pinned solves lifetime, not concurrent mutation.

## Dispatch checklist

1. **ADR-074 / SMP:** confirmed ACTIVATE and RECEIVE rights remain disjoint and syscall lookup rejects mixed/activate-only handles. Activation is serialized with enqueue. Found the lock-contention loss above. Pumps run in syscall context; the per-device `USB_DEVICE` lock serializes drains, but the present design chooses loss rather than waiting on `DELIVERY`. Existing flood coverage pumps then drains sequentially and does not exercise the two-CPU lock schedule.
2. **Shared pixels:** dimensions/stride are validated before constructing the compositor view; `compose_layers_xrgb8888` validates every layer and clips damage, so hostile pixel values cannot create an indexing panic. Pmap borrowed leaves pin an `Arc<SharedPixelBuffer>`, so client handle destruction or process exit cannot free mapped frames under the WM. Finding 2 remains for concurrent mutation of the ordinary slice.
3. **GpuSubmit handoff:** enforcement is armed before `spawn_neo_init`; APs are online but are not handed the cap. Arthur's inherited list contains the session channel, two channel endpoints, and optional directory only, and the negative scanout probe exercises the no-cap case. A process cannot self-mint `GpuSubmit`. Strictly, “only neo-init” is not a non-delegation invariant: `full_gpu_submit_rights()` includes DERIVE and GRANT, so trusted neo-init can intentionally derive/grant SUBMIT authority. The shipped path does not do so; this is a design note, not a separate defect.
4. **`SYS_FB_OUTPUT_QUERY`:** `0x1B0` is unique in the kernel/libneo tables. Width occupies bits 63..32 and is capped at `i32::MAX`, so success remains nonnegative; height is likewise capped and unpacking is inverse. Authority uses the same `holds_gpu_submit()` predicate as present (USE + SUBMIT). Runtime-off returns ENOSYS before other checks; missing cap returns EPERM; absent/zero framebuffer with a cap returns EINVAL. No extra information reaches a non-holder.
5. **Production vs scenario:** production initializes the framebuffer and arms Software present plus enforcement before user code. It does so after USB/network boot activity and AP bring-up, whereas the scenario supplies its own gate setup; this is a coverage gap, but I found no second initialization/state corruption in the inspected framebuffer path. The post-session framebuffer initialization is fallback-only.
6. **Kill-switch:** S1 additions in the kernel, neo-init, and Arthur are feature-gated; the fallback `run_selected_scenario` exclusions are present. The merged commit records the default `.text` equality claim; I did not rebuild both binaries. No unguarded S1 runtime call was found.
7. **Discriminator:** the full-screen readback is a closed-loop oracle because it mounts the same scene implementation. The hand-laid scene/layout/blend KATs break that loop for selected pixels and arithmetic, and the whole-frame digest plus HID serial script rejects a never-presented WM, stale BOOT indicator, or missing/misrouted typed sequence. It does not create the SMP delivery-lock contention in finding 1.
8. **Other:** the u16 wire bound is checked before layout/composition; outputs above 65535 fail the session cleanly rather than truncating damage. Mapping lifetime and raw-scanout authority are fail-closed. No additional CP-14/15/16/17/34/37 or I-14 defect survived re-verification.

## Validation performed

Static adversarial review only; no QEMU build was run. The key failing schedule is a concurrency property not covered by the existing single-pumper scenario.
