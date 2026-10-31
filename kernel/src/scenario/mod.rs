// SPDX-License-Identifier: MPL-2.0
//! Scenario-visible status hooks.

pub mod phase_indicator;

pub use phase_indicator::ScenarioPhase;

pub fn phase(phase: ScenarioPhase) {
    phase_indicator::set_phase(phase);
    crate::log::flush();
}
