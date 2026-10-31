// SPDX-License-Identifier: MPL-2.0
//! SAE (dragonfly) protocol state machine — the on-air driver over the stateless
//! `sae.rs` crypto (Phase 7.5 CB-5a ISC-1). `sae.rs` is pure primitives; this
//! wraps them in the SAE finite-state machine (Nothing → Committed → Confirmed →
//! Accepted) and produces/consumes the **SAE bodies** that ride in the 802.11
//! Authentication frame (the `mlme_logic::build_sae_*_auth` wrapper prepends
//! `alg=SAE(3) ‖ transaction ‖ status`).
//!
//! **H2E only** — matches `sae.rs` (hunting-and-pecking is deliberately absent),
//! so every Commit carries status `SAE_HASH_TO_ELEMENT` (126) and the
//! anti-clogging token, when present, rides the H2E **Anti-Clogging Token
//! Container** (IE 255, ext-id 93), never the inline hunting-and-pecking slot
//! (trap-mine T-051, `RESEARCH-2026-06-29-cb-5-sae-pmf-trap-mine-FINDINGS.md`).
//!
//! Byte-oracle facts baked in (T-049/T-050): the Commit body is the 98-octet
//! `group(2 LE) ‖ scalar(32) ‖ x(32) ‖ y(32)` (`sae::serialize_commit`); the
//! Confirm body is `send-confirm(2 LE) ‖ CN(32)`. Host-KAT'd in `ai-ops-tests`.
//!
//! Compiled only under the `wifi` feature.

extern crate alloc;

use alloc::vec::Vec;

use p256::{ProjectivePoint, Scalar};

use super::sae::{self, COMMIT_LEN, Commit, GROUP_P256, SaeKeys};

/// SAE status code: H2E PWE method (IEEE 802.11-2020 Table 9-50; ieee80211.h
/// `WLAN_STATUS_SAE_HASH_TO_ELEMENT`). The Commit status the STA and AP MUST
/// agree on — a 0-vs-126 mismatch is silently discarded (trap-mine T-052).
pub const SAE_HASH_TO_ELEMENT: u16 = 126;
/// SAE status code: the AP requests an anti-clogging token (T-051).
pub const ANTI_CLOG_REQUIRED: u16 = 76;

/// The Confirm body length: send-confirm(2) ‖ CN over SHA-256 (32).
pub const CONFIRM_BODY_LEN: usize = 34;

/// IE 255 = Element ID Extension; the H2E Anti-Clogging Token Container carries
/// ext-id 93 (trap-mine T-051, iwd `src/sae.c:975`).
const EID_EXTENSION: u8 = 255;
const EXT_ID_ANTI_CLOGGING_TOKEN: u8 = 93;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaeState {
    Nothing,
    Committed,
    Confirmed,
    Accepted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaeError {
    /// PWE/commit could not be derived (rand/mask out of range, point at ∞).
    Crypto,
    /// The peer Commit body is malformed (bad length/group/scalar/off-curve).
    BadCommit,
    /// The peer Confirm body is malformed (bad length).
    BadConfirm,
    /// The peer Confirm hash did not verify.
    ConfirmMismatch,
    /// An anti-clogging container was malformed.
    BadAntiClogging,
    /// A method call was made in the wrong state.
    WrongState,
}

/// One SAE protocol instance (initiator role — the STA). Drives `sae.rs`.
pub struct SaeSession {
    state: SaeState,
    pwe: ProjectivePoint,
    /// The per-commit `rand` (kept to derive `k` after the peer Commit arrives).
    rand: [u8; 32],
    own_scalar: Scalar,
    own_element: ProjectivePoint,
    peer_scalar: Option<Scalar>,
    peer_element: Option<ProjectivePoint>,
    keys: Option<SaeKeys>,
    /// Our send-confirm counter (starts at 1, IEEE 802.11-2020 §12.4.8.6.4).
    send_sc: u16,
    /// Anti-clogging token, empty until the AP requests one (T-051).
    token: Vec<u8>,
}

impl SaeSession {
    /// Derive PT → PWE → commit (scalar, element) for `(ssid, password,
    /// identifier)` bound to `(own_mac, peer_mac)`. `rand`/`mask` are the
    /// per-commit CSPRNG scalars (the caller supplies them so the session is
    /// deterministic under test). Returns `None` if the commit is degenerate.
    pub fn new(
        ssid: &[u8],
        password: &[u8],
        identifier: &[u8],
        own_mac: &[u8; 6],
        peer_mac: &[u8; 6],
        rand: &[u8; 32],
        mask: &[u8; 32],
    ) -> Result<SaeSession, SaeError> {
        let pt = sae::derive_pt(ssid, password, identifier);
        let pwe = sae::derive_pwe(&pt, own_mac, peer_mac);
        let own_scalar = sae::commit_scalar(rand, mask).ok_or(SaeError::Crypto)?;
        let own_element = sae::commit_element(&pwe, mask).ok_or(SaeError::Crypto)?;
        // Reject a degenerate commit-scalar/element up front (parse_commit would
        // reject the wire form too, but fail fast on the local side).
        sae::serialize_commit(GROUP_P256, &own_scalar, &own_element).ok_or(SaeError::Crypto)?;
        Ok(SaeSession {
            state: SaeState::Nothing,
            pwe,
            rand: *rand,
            own_scalar,
            own_element,
            peer_scalar: None,
            peer_element: None,
            keys: None,
            send_sc: 1,
            token: Vec::new(),
        })
    }

    pub fn state(&self) -> SaeState {
        self.state
    }

    /// The Commit status code (always H2E — this stack is H2E-only, T-052).
    pub fn commit_status(&self) -> u16 {
        SAE_HASH_TO_ELEMENT
    }

    /// The SAE **Commit body** to place after `alg ‖ transaction(1) ‖ status`:
    /// the 98-octet `serialize_commit` output, followed — only after the AP has
    /// requested one — by the H2E Anti-Clogging Token Container (T-051).
    pub fn commit_body(&self) -> Result<Vec<u8>, SaeError> {
        let commit = sae::serialize_commit(GROUP_P256, &self.own_scalar, &self.own_element)
            .ok_or(SaeError::Crypto)?;
        let mut body = Vec::with_capacity(COMMIT_LEN + self.token.len() + 3);
        body.extend_from_slice(&commit);
        if !self.token.is_empty() {
            // Anti-Clogging Token Container: [255][len][93][token...], len = 1+token.
            body.push(EID_EXTENSION);
            body.push((self.token.len() + 1) as u8);
            body.push(EXT_ID_ANTI_CLOGGING_TOKEN);
            body.extend_from_slice(&self.token);
        }
        Ok(body)
    }

    /// Process the peer's Commit body (the bytes after `alg ‖ transaction ‖
    /// status`). Parses the 98-octet commit, derives the shared secret and the
    /// KCK/PMK, and advances to Committed. Trailing IEs (rejected-groups,
    /// password-identifier) are ignored — as initiator we send none and, per iwd
    /// `sae.c:1207-1216`, must not receive them, but tolerating extra bytes is
    /// safe (the crypto uses only the fixed 98).
    pub fn on_peer_commit(&mut self, body: &[u8]) -> Result<(), SaeError> {
        if !matches!(self.state, SaeState::Nothing) {
            return Err(SaeError::WrongState);
        }
        if body.len() < COMMIT_LEN {
            return Err(SaeError::BadCommit);
        }
        let Commit {
            scalar: peer_scalar,
            element: peer_element,
            ..
        } = sae::parse_commit(&body[..COMMIT_LEN]).ok_or(SaeError::BadCommit)?;
        let k = sae::derive_k(&self.pwe, &peer_scalar, &peer_element, &self.rand)
            .ok_or(SaeError::Crypto)?;
        let keys = sae::derive_keys(&k, &self.own_scalar, &peer_scalar);
        self.peer_scalar = Some(peer_scalar);
        self.peer_element = Some(peer_element);
        self.keys = Some(keys);
        self.state = SaeState::Committed;
        Ok(())
    }

    /// The SAE **Confirm body** to place after `alg ‖ transaction(2) ‖ status(0)`:
    /// `send-confirm(2 LE) ‖ CN(32)`. Requires Committed state (keys derived).
    pub fn confirm_body(&self) -> Result<Vec<u8>, SaeError> {
        let keys = self.keys.as_ref().ok_or(SaeError::WrongState)?;
        let peer_scalar = self.peer_scalar.as_ref().ok_or(SaeError::WrongState)?;
        let peer_element = self.peer_element.as_ref().ok_or(SaeError::WrongState)?;
        let cn = sae::confirm(
            &keys.kck,
            self.send_sc,
            &self.own_scalar,
            &self.own_element,
            peer_scalar,
            peer_element,
        )
        .ok_or(SaeError::Crypto)?;
        let mut body = Vec::with_capacity(CONFIRM_BODY_LEN);
        body.extend_from_slice(&self.send_sc.to_le_bytes());
        body.extend_from_slice(&cn);
        Ok(body)
    }

    /// Process the peer's Confirm body (`send-confirm(2 LE) ‖ CN(32)`). Recomputes
    /// the peer's CN with the scalars/elements SWAPPED (the peer's own values
    /// first) and constant-time-compares. On match, the PMK is Accepted.
    pub fn on_peer_confirm(&mut self, body: &[u8]) -> Result<(), SaeError> {
        if !matches!(self.state, SaeState::Committed | SaeState::Confirmed) {
            return Err(SaeError::WrongState);
        }
        if body.len() < CONFIRM_BODY_LEN {
            return Err(SaeError::BadConfirm);
        }
        let keys = self.keys.as_ref().ok_or(SaeError::WrongState)?;
        let peer_scalar = self.peer_scalar.as_ref().ok_or(SaeError::WrongState)?;
        let peer_element = self.peer_element.as_ref().ok_or(SaeError::WrongState)?;
        let peer_sc = u16::from_le_bytes([body[0], body[1]]);
        let expected = sae::confirm(
            &keys.kck,
            peer_sc,
            peer_scalar,
            peer_element,
            &self.own_scalar,
            &self.own_element,
        )
        .ok_or(SaeError::Crypto)?;
        // Constant-time compare (eapol.rs house style: XOR-accumulate).
        let mut diff = 0u8;
        for i in 0..32 {
            diff |= expected[i] ^ body[2 + i];
        }
        if diff != 0 {
            return Err(SaeError::ConfirmMismatch);
        }
        self.state = SaeState::Accepted;
        Ok(())
    }

    /// Handle an AP Commit reply with status `ANTI_CLOG_REQUIRED` (76). The body
    /// after `alg ‖ transaction(1) ‖ status(76)` is `group(2 LE) ‖ container`,
    /// where the container is IE 255 / ext-id 93 / token (T-051). Stores the
    /// token so the next `commit_body()` carries it, and stays in Nothing to
    /// re-send Commit (one retry, driven by the caller).
    pub fn on_anti_clogging(&mut self, body: &[u8]) -> Result<(), SaeError> {
        if !matches!(self.state, SaeState::Nothing) {
            return Err(SaeError::WrongState);
        }
        // group(2 LE) then the Anti-Clogging Token Container.
        if body.len() < 2 + 3 {
            return Err(SaeError::BadAntiClogging);
        }
        let group = u16::from_le_bytes([body[0], body[1]]);
        if group != GROUP_P256 {
            return Err(SaeError::BadAntiClogging);
        }
        let ie = &body[2..];
        // [255][len][93][token...], len = 1 + token_len.
        if ie[0] != EID_EXTENSION || ie[2] != EXT_ID_ANTI_CLOGGING_TOKEN || (ie[1] as usize) < 2 {
            return Err(SaeError::BadAntiClogging);
        }
        let token_len = (ie[1] as usize) - 1;
        if ie.len() < 3 + token_len || token_len == 0 || token_len > 256 {
            return Err(SaeError::BadAntiClogging);
        }
        self.token.clear();
        self.token.extend_from_slice(&ie[3..3 + token_len]);
        Ok(())
    }

    /// The derived PMK, once Accepted (feeds the 4-way as `target.pmk`, ISC-4).
    pub fn pmk(&self) -> Option<[u8; 32]> {
        match self.state {
            SaeState::Accepted => self.keys.as_ref().map(|k| k.pmk),
            _ => None,
        }
    }

    /// The PMKID (context[..16]) — for the RSNE PMKID list / logging.
    pub fn pmkid(&self) -> Option<[u8; 16]> {
        self.keys.as_ref().map(|k| k.pmkid)
    }
}
