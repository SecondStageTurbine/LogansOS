// SPDX-License-Identifier: MPL-2.0
//! W2: the in-kernel 802.11 STA supplicant — the MLME state machine, the 4-way
//! handshake wiring, the CCMP data path, and the NetDevice seam. Built on W1's
//! MT7601U frames and W0/W0b's supplicant crypto (`crate::net::wpa`).
//!
//! mac80211 leaves the 4-way handshake to a userspace supplicant; NeoPlutarch
//! has none, so this is an in-kernel minimal supplicant. Default-off behind the
//! `wifi` feature (the W2 kill-switch: disabled ≡ W1 scan-only).

pub mod mlme_logic;
/// CB-5a: the WPA3-SAE connect orchestrator (SAE→assoc→4-way→GKH→BIP) over an
/// abstract TX/RX seam. Default-off behind `wpa3-sae` (off ≡ CB-2/WPA2).
#[cfg(feature = "wpa3-sae")]
pub mod sae_connect;
