// SPDX-License-Identifier: MPL-2.0
//! Atrium G1 compositor slice-1 (Rung 0 of the OT-63 accel ladder).
//!
//! Exactly the two ratified kernel object kinds enter ring 0 here
//! (`docs/THINK-2026-07-10-g1-compositor-SYNTHESIS.md` §4):
//!
//! - [`buffer::SharedPixelBuffer`] — the shared pixel-buffer object:
//!   immutable descriptor, kernel-owned frames, client RW / compositor
//!   RO+NX dual mapping via borrowed Pmap leaves, and the embedded
//!   software present fence (epoch-published after the real blit).
//! - [`focus::InputFocus`] — kernel-routed atomic input-focus transfer.
//!
//! Everything else (scene, registry, damage rects, pacing, protocol verbs)
//! is userland compositor state in later slices. This module is fully gated
//! behind the default-off `atrium-compositor-g1` feature: the feature-off
//! kernel is bit-for-bit today's kernel (the Rule-1 kill-switch).

pub mod blur;
pub mod buffer;
pub mod focus;
pub mod glass;
pub mod input_logic;
pub mod input_queue_logic;
#[cfg(feature = "key-autorepeat")]
pub mod key_repeat;
#[cfg(feature = "key-autorepeat")]
pub mod key_repeat_logic;
pub mod logic;
pub mod pointer_route_logic;
pub mod present_backend;
pub mod sak_logic;

use core::sync::atomic::{AtomicBool, Ordering};

/// Runtime half of the glass-blur kill-switch (ADR-077 v2 §5.1 / OT-63 GPU P1
/// Slice 1). Default **off**: the backdrop passes through as the solid
/// Reduce-Transparency floor — bit-for-bit today's composite. Flipped on only by
/// the `atrium_glass_blur` scenario (and, later, the ratified glass compositor
/// slice). Mirrors `present_backend`'s `SCANOUT_FENCE_ENABLED`/`GPU_SUBMIT_ENFORCE`
/// toggle shape.
#[allow(dead_code)]
static GLASS_BLUR_ENABLE: AtomicBool = AtomicBool::new(false);

/// Arm/disarm the CPU glass blur (see [`GLASS_BLUR_ENABLE`]). Slice-1 exercises
/// this from the `atrium_glass_blur` scenario; Slice 6 wires it into the live
/// glass compositor — so it is dead in other atrium builds.
#[allow(dead_code)]
pub fn set_glass_blur_enabled(enabled: bool) {
    GLASS_BLUR_ENABLE.store(enabled, Ordering::Release);
}

/// Whether the CPU glass blur runs: the two-layer kill-switch
/// ([`blur::glass_blur_enabled`]) over the compile-time compositor feature AND
/// the runtime toggle. Off ⇒ `blur_backdrop` is a bit-for-bit passthrough.
#[allow(dead_code)]
#[inline]
pub fn glass_blur_enabled() -> bool {
    blur::glass_blur_enabled(
        cfg!(feature = "atrium-compositor-g1"),
        GLASS_BLUR_ENABLE.load(Ordering::Acquire),
    )
}

/// The runtime half of the two-layer kill-switch (chad §8.1:
/// `atrium_compositor=off` until the G1 build is ratified). Slice-1 turns it
/// on only inside the compose scenario build; every other build — including
/// the kill-switch equality scenario, which compiles the feature IN — keeps
/// it off and proves the object syscalls are unreachable (`ENOSYS`, zero
/// objects minted).
// Rule-4 consolidation (2026-07-20): the per-slice `any(feature = ...)` list that
// every G1 slice extended by hand (and that dropped a comma in a co-residence
// merge) is now the single `neo-test-atrium-runtime` umbrella marker. A scenario
// joins the runtime-on set by depending on that feature in Cargo.toml — one
// append on its own feature line — never by editing this expression. The set is
// provably identical: exactly slice1/surface-protocol/input-routing/interactive/
// slice3 enable `neo-test-atrium-runtime` (see kernel/Cargo.toml).
// Desktop S1: the production `desktop-session` build is the one non-scenario
// configuration that turns the runtime switch on (the session compositor IS the
// G1 compositor). Feature-off ⇒ this is exactly the scenario-only expression.
pub const ATRIUM_RUNTIME_ON: bool = cfg!(any(
    feature = "neo-test-atrium-runtime",
    feature = "desktop-session"
));

/// Gate consulted by every atrium syscall before touching any object state.
#[inline]
pub fn syscalls_enabled() -> bool {
    logic::atrium_syscalls_enabled(true, ATRIUM_RUNTIME_ON)
}

#[inline]
pub fn input_routing_enabled() -> bool {
    input_logic::routing_enabled(cfg!(feature = "atrium-input-routing"), syscalls_enabled())
}
