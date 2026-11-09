// SPDX-License-Identifier: MPL-2.0
//! DS-1b: the session broker's LaunchCap table (pure; host-tested).
//!
//! **ADVISORY.** The kernel still lets any process `sys_spawn` any module by
//! name, so a LaunchCap governs only launches that go through the broker. The
//! kernel `SpawnCap` gate (OT-66) is a separate rung; the boot test pins
//! today's status by asserting that a direct `sys_spawn` still succeeds.
//!
//! A LaunchCap is a channel endpoint the broker mints for one client surface
//! with an allowlist of module ids. To launch, the client sends `OP_LAUNCH`
//! carrying a token on its surface channel, then sends the same token on its
//! LaunchCap endpoint; the broker receives it on its own end (IPC is
//! synchronous rendezvous, ADR-004). Possession is proven by the CHANNEL:
//! only a holder of the client end can send on it. The token only ties that
//! send to this request (the client chooses it), so a mismatch means the
//! holder's proof belongs to some other request and is refused.
//! The broker never receives on a handle a client gave it. Waiting on its own
//! end right after that client's request is the same exposure as waiting on
//! the client's surface channel (the recorded DS-1 focus-trap class).

use crate::atrium_protocol::{LaunchRequest, ProtocolError};

/// Module ids (an enumeration, never a free-form name: no name injection).
pub const MODULE_ARTHUR: u32 = 1;
/// A harmless probe (`neo-worker` with no handles exits at once).
pub const MODULE_PROBE: u32 = 2;
/// DS-1c: the Lens, a read-only directory instrument. The same `neo-worker`
/// binary (the repo's multi-role child). Which role a `neo-worker` build
/// runs is chosen at compile time, by cargo feature (`desktop-lens`; see
/// neo-worker's `_start`), not at spawn. What the broker hands it at spawn
/// is what the Lens may use: a session channel and one `LOOKUP | READDIR`
/// directory handle, nothing else.
pub const MODULE_LENS: u32 = 3;
/// OT-88 A1a: the probe app, a second focusable client for the app-model
/// scenarios (`neo-worker` built with `probe-app`). Ids 4-7 are the apps'
/// (`docs/THINK-2026-10-29-ot88-app-model.md` §2.3, item 5).
pub const MODULE_PROBE_APP: u32 = 8;
/// L1a.3: the terminal window, `neo-worker` built with `terminal-app`
/// (`docs/THINK-2026-10-31-l1a-terminal-window.md` §2). Its spawn name is
/// `worker` only with libneo's `atrium-terminal`: off, the id names nothing,
/// so the broker refuses it (§7's kill-switch).
pub const MODULE_TERMINAL: u32 = 4;
/// The terminal's surface namespace and buffer provenance.
pub const TERMINAL_NAMESPACE: u64 = 0x5445_524D_0000_0001;
pub const TERMINAL_BUFFER_PROVENANCE: u64 = 0x5445_524D_4255_0001;
/// The probe app's surface namespace and buffer provenance (the WM's
/// registry for it, and the import it accepts, are keyed by these).
pub const PROBE_APP_NAMESPACE: u64 = 0x5052_4F42_4150_0001;
pub const PROBE_APP_BUFFER_PROVENANCE: u64 = 0x5052_4F42_4255_0001;
/// The Lens's surface namespace and client size (the WM lays it out at
/// this size; neo-atrium's `LENS_W`).
pub const LENS_NAMESPACE: u64 = 0x4C45_4E53_0000_0001;
pub const LENS_WIDTH: u32 = 360;
pub const LENS_HEIGHT: u32 = 400;

/// The `sys_spawn` name of a module id.
pub const fn module_name(module_id: u32) -> Option<&'static str> {
    match module_id {
        MODULE_ARTHUR => Some("arthur"),
        MODULE_PROBE | MODULE_LENS | MODULE_PROBE_APP => Some("worker"),
        #[cfg(feature = "atrium-terminal")]
        MODULE_TERMINAL => Some("worker"),
        _ => None,
    }
}

/// The allowlist bit of a module id.
pub const fn allow(module_id: u32) -> u32 {
    if module_id == 0 || module_id > 31 {
        0
    } else {
        1 << module_id
    }
}

pub const MAX_LAUNCH_CAPS: usize = 4;

/// Chad DS-1c F1: how many Lens blooms one client session may attempt. A
/// bloom that fails spends its attempt like one that succeeds, so a client
/// cannot retry without bound (each attempt spawns a process).
pub const MAX_LENS_ATTEMPTS: u32 = 3;
/// `OP_LAUNCH` status when the session's Lens attempts are spent.
pub const STATUS_ATTEMPTS_SPENT: u32 = 12;

/// The per-session Lens attempt budget (the WM holds one per client).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LensAttempts {
    used: u32,
    shown: bool,
}

/// What a Lens request may do now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LensAttempt {
    /// Try to bloom: attempt `n` (1-based) of [`MAX_LENS_ATTEMPTS`].
    Bloom(u32),
    /// A Lens is already shown this session (one per session).
    AlreadyShown,
    /// Every attempt is spent, successful or not.
    Spent,
}

impl LensAttempts {
    /// Spend an attempt (before blooming: a failure spends it too).
    pub fn begin(&mut self) -> LensAttempt {
        if self.shown {
            return LensAttempt::AlreadyShown;
        }
        if self.used >= MAX_LENS_ATTEMPTS {
            return LensAttempt::Spent;
        }
        self.used += 1;
        LensAttempt::Bloom(self.used)
    }

    /// The attempt in progress bloomed.
    pub fn shown(&mut self) {
        self.shown = true;
    }

    pub fn used(&self) -> u32 {
        self.used
    }
}

/// Reply statuses of `OP_LAUNCH` beyond the ADR-073 protocol errors (1..=9):
/// the module is not on the cap's allowlist, or the broker's spawn failed.
pub const STATUS_NOT_PERMITTED: u32 = 10;
pub const STATUS_SPAWN_FAILED: u32 = 11;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LaunchCap {
    surface_cap_provenance: u64,
    allowlist: u32,
}

/// Why a launch was refused (mapped onto the protocol status domain).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaunchRefusal {
    /// No LaunchCap is bound to this surface, the request named another
    /// surface, or the token on the LaunchCap did not match (no proof of
    /// possession).
    CapRequired,
    /// The module is not on this LaunchCap's allowlist (or does not exist).
    NotPermitted,
}

impl LaunchRefusal {
    pub const fn status(self) -> u32 {
        match self {
            Self::CapRequired => ProtocolError::CapRequired.status(),
            Self::NotPermitted => STATUS_NOT_PERMITTED,
        }
    }
}

/// The broker's table of minted LaunchCaps.
pub struct LaunchBroker {
    caps: [Option<LaunchCap>; MAX_LAUNCH_CAPS],
}

impl Default for LaunchBroker {
    fn default() -> Self {
        Self::new()
    }
}

impl LaunchBroker {
    pub const fn new() -> Self {
        Self {
            caps: [None; MAX_LAUNCH_CAPS],
        }
    }

    /// Mint a LaunchCap bound to `surface_cap_provenance`; `false` when the
    /// table is full.
    pub fn mint(&mut self, surface_cap_provenance: u64, allowlist: u32) -> bool {
        match self.caps.iter_mut().find(|c| c.is_none()) {
            Some(slot) => {
                *slot = Some(LaunchCap {
                    surface_cap_provenance,
                    allowlist,
                });
                true
            }
            None => false,
        }
    }

    /// Drop the LaunchCap bound to a surface (its client ended).
    pub fn revoke(&mut self, surface_cap_provenance: u64) {
        for slot in &mut self.caps {
            if slot.is_some_and(|c| c.surface_cap_provenance == surface_cap_provenance) {
                *slot = None;
            }
        }
    }

    /// Judge a launch received on the channel whose provenance is
    /// `received_cap_provenance`, given the token the broker then received on
    /// its own end of that client's LaunchCap channel (`presented`; only the
    /// endpoint's holder can send it). Returns the module's spawn name.
    pub fn judge(
        &self,
        request: LaunchRequest,
        received_cap_provenance: u64,
        presented: Option<u64>,
    ) -> Result<&'static str, LaunchRefusal> {
        let cap = self
            .caps
            .iter()
            .flatten()
            .find(|c| c.surface_cap_provenance == received_cap_provenance)
            .ok_or(LaunchRefusal::CapRequired)?;
        // Token 0 is refused here too, not only by the decoder: the pure
        // API fails closed for a directly constructed request (Janus DS-1b).
        if request.identity.surface_cap_provenance != received_cap_provenance
            || request.launch_token == 0
            || presented != Some(request.launch_token)
        {
            return Err(LaunchRefusal::CapRequired);
        }
        if cap.allowlist & allow(request.module_id) == 0 {
            return Err(LaunchRefusal::NotPermitted);
        }
        module_name(request.module_id).ok_or(LaunchRefusal::NotPermitted)
    }
}
