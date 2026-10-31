// SPDX-License-Identifier: MPL-2.0
//! Glass material and trust-state-as-tint (ADR-024 Amendment 2026-07-19 §2).
//!
//! Only `Owned` is rendered today; the table carries every ratified state so
//! the others slot in. Hue is never the only channel: each state also has a
//! [`CapHalo`] geometry cue that survives Reduce-Transparency unchanged.

use crate::wide::{WHITE_W, Wide, widen};

/// Electric Blue: light, never fill.
pub const ELECTRIC_BLUE: u32 = 0x003C_8EF4;
/// Muted, structure-inactive blue (unfocused accents).
pub const INACTIVE_BLUE: u32 = 0x0024_4A7A;
/// Alert Magenta: denial only.
pub const MAGENTA: u32 = 0x00FF_00FF;

/// Reduce-Transparency opaque neutral for panels (contrast-tested, see
/// [`crate::contrast`]).
pub const RT_PANEL: u32 = 0x001A_2130;
/// Reduce-Transparency opaque neutral for raised tiles (the Spine chip).
pub const RT_RAISED: u32 = 0x0022_2A3C;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Mode {
    /// Frosted glass over the cached backdrop blur.
    Glass,
    /// The kill-switch + accessibility floor: opaque neutral + the same cues.
    ReduceTransparency,
}

/// The ratified cap-surface trust states (design language §4).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrustState {
    Owned,
    Tethered,
    UndoArmed,
    Denied,
    Untrusted,
}

/// The hue-independent, mode-surviving cue for a trust state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapHalo {
    /// Solid hairline + corner ticks.
    SolidTicks,
    /// Dashed hairline + a tether line to the grant.
    DashedTether,
    /// Solid hairline + ticks, with the Undo Shadow armed.
    SolidTicksUndo,
    /// Solid hairline + a left edge bar and a "grant required" chip.
    EdgeBarChip,
    /// Stippled hairline, not activatable.
    Stipple,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Tint {
    /// White frost weight out of 256.
    pub frost: u32,
    /// Vibrancy weight out of 256.
    pub vibrancy: u32,
    /// Vibrancy colour.
    pub vibrancy_color: u32,
    pub halo: CapHalo,
}

impl TrustState {
    pub const fn tint(self) -> Tint {
        match self {
            TrustState::Owned => Tint {
                frost: 18,
                vibrancy: 26,
                vibrancy_color: ELECTRIC_BLUE,
                halo: CapHalo::SolidTicks,
            },
            TrustState::Tethered => Tint {
                frost: 18,
                vibrancy: 26,
                vibrancy_color: ELECTRIC_BLUE,
                halo: CapHalo::DashedTether,
            },
            TrustState::UndoArmed => Tint {
                frost: 18,
                vibrancy: 26,
                vibrancy_color: ELECTRIC_BLUE,
                halo: CapHalo::SolidTicksUndo,
            },
            TrustState::Denied => Tint {
                frost: 64,
                vibrancy: 20,
                vibrancy_color: MAGENTA,
                halo: CapHalo::EdgeBarChip,
            },
            TrustState::Untrusted => Tint {
                frost: 96,
                vibrancy: 0,
                vibrancy_color: 0,
                halo: CapHalo::Stipple,
            },
        }
    }
}

/// The glass colour over a blurred backdrop sample `b` (wide units):
/// `(b·(256−F−V) + 65280·F + 256·C·V + 128) >> 8` per channel.
pub fn glass(b: Wide, frost: u32, vibrancy: u32, vibrancy_color: u32) -> Wide {
    let v = widen(vibrancy_color);
    let keep = 256 - frost - vibrancy;
    let ch = |i: usize| (b[i] * keep + WHITE_W * frost + v[i] * vibrancy + 128) >> 8;
    [ch(0), ch(1), ch(2)]
}
