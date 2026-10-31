// SPDX-License-Identifier: MPL-2.0
//! The WPA3-SAE connect orchestrator (Phase 7.5 CB-5a). Drives the **full**
//! on-air sequence — SAE Commit/Confirm → PMF-negotiated Assoc → SAE-PMK 4-way →
//! IGTK/BIP → Group-Key-Handshake → forged-deauth-ignored — over an abstract
//! `ConnectSeam` (management/EAPOL TX+RX + key install + CSPRNG). The seam is the
//! test/hardware boundary: the hermetic QEMU gate drives a scripted in-process AP
//! through it (so this whole state machine is gate-proven with no radio); CB-5b
//! supplies the thin real-mt7601u adapter for the on-air run.
//!
//! Reuses the shipped logic — `sae_sm` (SAE FSM), `mlme_logic` (frames),
//! `eapol`/`kdf`/`keywrap` (4-way), `bip` (802.11w) — and adds only the
//! sequencing. Every §5 breadcrumb is emitted via `seam.breadcrumb` so CB-5b's
//! on-air run is self-diagnosing (capture-before-you-need-it).
//!
//! Compiled only under the `wpa3-sae` feature (off ⇒ the connect path is WPA2/
//! CB-2, bit-for-bit — this module is not built).

extern crate alloc;

use core::sync::atomic::{AtomicBool, Ordering};

use crate::net::wpa::{bip, eapol, kdf, keywrap, sae_sm};

use super::mlme_logic;

/// ISC-5 runtime kill-switch (mirrors `mt7601u::GROUP_RX_ENABLED`). `false` ⇒ the
/// driver's `associate()` never enters the SAE path and the connect behaves
/// exactly as the CB-2/WPA2 baseline. This is the *within-build* half of the
/// kill-switch; the compile-time half is the `wpa3-sae` feature gate (off ⇒ this
/// whole module is not built). Default **OFF**: SAE is opt-in even when compiled,
/// so a `wpa3-sae` kernel that never flips this is byte-identical to CB-2 on the
/// connect path — that equality is exactly what the ISC-5 KAT asserts.
static SAE_ENABLED: AtomicBool = AtomicBool::new(false);

/// Flip the ISC-5 runtime kill-switch (the ISC-5 scenario + the CB-5b adapter).
#[allow(dead_code)] // consumed by the gated associate() hook + the ISC-5 scenario
pub fn set_sae_enabled(enabled: bool) {
    SAE_ENABLED.store(enabled, Ordering::Release);
}

/// Read the ISC-5 runtime kill-switch. The gated `associate()` hook checks this
/// before ever touching the SAE path.
#[allow(dead_code)]
pub fn sae_enabled() -> bool {
    SAE_ENABLED.load(Ordering::Acquire)
}

/// §5 breadcrumbs — the discriminator points CB-5b's on-air run keys off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectEvent {
    SaeCommitTx,
    AntiCloggingRetry,
    SaeCommitRx,
    SaeConfirmTx,
    SaeConfirmRx,
    PmkDerived,
    AssocResp,
    FourWayM1,
    FourWayM2,
    FourWayM3,
    GtkInstalled,
    IgtkInstalled,
    FourWayM4,
    GroupRekeyed,
    ForgedDeauthDropped,
    ProtectedDeauthAccepted,
}

/// Errors from the abstract seam (TX/RX/install failures).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeamError {
    Tx,
    Rx,
    Install,
}

/// Where in the sequence a connect attempt failed — each maps to a §5 row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectError {
    Seam(SeamError),
    Sae(sae_sm::SaeError),
    /// The SAE session did not reach a PMK.
    NoPmk,
    /// The Association Response carried a non-zero status.
    AssocRejected(u16),
    /// A received frame was malformed at the given stage.
    BadFrame(ConnectEvent),
    /// A 4-way / GKH EAPOL MIC did not verify.
    MicFail(ConnectEvent),
    Bip(bip::BipError),
}

impl From<SeamError> for ConnectError {
    fn from(e: SeamError) -> Self {
        ConnectError::Seam(e)
    }
}

/// The abstract management/EAPOL wire the orchestrator drives. The hermetic gate
/// implements it with a scripted AP peer; CB-5b implements it over the real
/// mt7601u `tx_frame`/`poll_mgmt`/`tx_eapol`/`poll_eapol` (the thin adapter).
pub trait ConnectSeam {
    fn tx_mgmt(&mut self, frame: &[u8]) -> Result<(), SeamError>;
    fn rx_mgmt(&mut self, buf: &mut [u8]) -> Result<usize, SeamError>;
    fn tx_eapol(&mut self, frame: &[u8]) -> Result<(), SeamError>;
    fn rx_eapol(&mut self, buf: &mut [u8]) -> Result<usize, SeamError>;
    fn install_pairwise(&mut self, tk: &[u8; 16]) -> Result<(), SeamError>;
    fn install_gtk(&mut self, gtk: &[u8], key_id: u8) -> Result<(), SeamError>;
    /// Fill `out` with CSPRNG bytes (SAE rand/mask, the SNonce).
    fn random(&mut self, out: &mut [u8]);
    fn breadcrumb(&mut self, ev: ConnectEvent);
}

/// The connect parameters (credentials + the peer's addresses).
pub struct ConnectConfig<'a> {
    pub ssid: &'a [u8],
    pub password: &'a [u8],
    pub identifier: &'a [u8],
    pub sta: [u8; 6],
    pub ap: [u8; 6],
}

/// The result of a successful WPA3-SAE connect (for the RETURN / logging).
#[derive(Debug, Clone)]
pub struct ConnectOutcome {
    pub pmk: [u8; 32],
    pub pmkid: [u8; 16],
    pub gtk_installed: bool,
    pub igtk: Option<bip::Igtk>,
    pub group_rekeyed: bool,
    pub forged_deauth_ignored: bool,
}

const BUF: usize = 512;

/// Run the full WPA3-SAE connect sequence over `seam`.
pub fn sae_connect<S: ConnectSeam>(
    seam: &mut S,
    cfg: &ConnectConfig,
) -> Result<ConnectOutcome, ConnectError> {
    // ── 1. SAE Commit/Confirm (ISC-1) ───────────────────────────────────────
    let mut rand = [0u8; 32];
    let mut mask = [0u8; 32];
    seam.random(&mut rand);
    seam.random(&mut mask);
    let mut session = sae_sm::SaeSession::new(
        cfg.ssid,
        cfg.password,
        cfg.identifier,
        &cfg.sta,
        &cfg.ap,
        &rand,
        &mask,
    )
    .map_err(ConnectError::Sae)?;

    let tx_commit = |seam: &mut S, session: &sae_sm::SaeSession| -> Result<(), ConnectError> {
        let body = session.commit_body().map_err(ConnectError::Sae)?;
        let mut frame = [0u8; BUF];
        let n = mlme_logic::build_sae_auth(
            &mut frame,
            &cfg.sta,
            &cfg.ap,
            1,
            session.commit_status(),
            &body,
        )
        .ok_or(ConnectError::BadFrame(ConnectEvent::SaeCommitTx))?;
        seam.tx_mgmt(&frame[..n])?;
        Ok(())
    };
    tx_commit(seam, &session)?;
    seam.breadcrumb(ConnectEvent::SaeCommitTx);

    // Receive the AP's reply; one anti-clogging retry (T-051).
    let mut rx = [0u8; BUF];
    let mut n = seam.rx_mgmt(&mut rx)?;
    let (_, status) =
        parse_auth_fixed(&rx[..n]).ok_or(ConnectError::BadFrame(ConnectEvent::SaeCommitRx))?;
    if status == sae_sm::ANTI_CLOG_REQUIRED {
        let body = mlme_logic::sae_auth_body(&rx[..n])
            .ok_or(ConnectError::BadFrame(ConnectEvent::SaeCommitRx))?;
        session.on_anti_clogging(body).map_err(ConnectError::Sae)?;
        tx_commit(seam, &session)?;
        seam.breadcrumb(ConnectEvent::AntiCloggingRetry);
        n = seam.rx_mgmt(&mut rx)?;
    }
    let body = mlme_logic::sae_auth_body(&rx[..n])
        .ok_or(ConnectError::BadFrame(ConnectEvent::SaeCommitRx))?;
    session.on_peer_commit(body).map_err(ConnectError::Sae)?;
    seam.breadcrumb(ConnectEvent::SaeCommitRx);

    // Confirm exchange.
    let cbody = session.confirm_body().map_err(ConnectError::Sae)?;
    let mut frame = [0u8; BUF];
    let cn = mlme_logic::build_sae_auth(&mut frame, &cfg.sta, &cfg.ap, 2, 0, &cbody)
        .ok_or(ConnectError::BadFrame(ConnectEvent::SaeConfirmTx))?;
    seam.tx_mgmt(&frame[..cn])?;
    seam.breadcrumb(ConnectEvent::SaeConfirmTx);

    let n = seam.rx_mgmt(&mut rx)?;
    let body = mlme_logic::sae_auth_body(&rx[..n])
        .ok_or(ConnectError::BadFrame(ConnectEvent::SaeConfirmRx))?;
    session.on_peer_confirm(body).map_err(ConnectError::Sae)?;
    seam.breadcrumb(ConnectEvent::SaeConfirmRx);

    let pmk = session.pmk().ok_or(ConnectError::NoPmk)?;
    let pmkid = session.pmkid().ok_or(ConnectError::NoPmk)?;
    seam.breadcrumb(ConnectEvent::PmkDerived);

    // ── 2. Association with the WPA3 RSNE (ISC-2) ────────────────────────────
    let mut areq = [0u8; BUF];
    let arn = build_assoc_req_sae(&mut areq, &cfg.sta, &cfg.ap, cfg.ssid)
        .ok_or(ConnectError::BadFrame(ConnectEvent::AssocResp))?;
    seam.tx_mgmt(&areq[..arn])?;
    let n = seam.rx_mgmt(&mut rx)?;
    let resp = mlme_logic::parse_assoc_resp(&rx[..n])
        .ok_or(ConnectError::BadFrame(ConnectEvent::AssocResp))?;
    if resp.status != 0 {
        return Err(ConnectError::AssocRejected(resp.status));
    }
    seam.breadcrumb(ConnectEvent::AssocResp);

    // ── 3. SAE-PMK → 4-way (ISC-4), descriptor v3 (AES-CMAC) ─────────────────
    let ptk = four_way(seam, cfg, &pmk)?;
    let (gtk_installed, igtk) = install_from_m3(seam, &ptk.m3_key_data)?;

    // ── 4. Group-Key-Handshake (ISC-4, key_info 0x1383) ──────────────────────
    let group_rekeyed = group_key_handshake(seam, &ptk.kck, &ptk.kek, ptk.version)?;

    // ── 5. BIP enforcement (ISC-3): a forged unprotected deauth is ignored ───
    let forged_deauth_ignored = bip_deauth_check(seam, &mut rx, &igtk)?;

    Ok(ConnectOutcome {
        pmk,
        pmkid,
        gtk_installed,
        igtk,
        group_rekeyed,
        forged_deauth_ignored,
    })
}

/// The 4-way outputs carried into the group-key + BIP stages. The pairwise TK is
/// installed inside `four_way` (via the seam) the moment M3 verifies, so it is
/// not re-carried here; the KCK/KEK drive the GKH and the M3 Key Data holds the
/// GTK/IGTK KDEs.
struct FourWayResult {
    kck: [u8; 16],
    kek: [u8; 16],
    version: eapol::KeyDescriptorVersion,
    m3_key_data: alloc::vec::Vec<u8>,
}

fn four_way<S: ConnectSeam>(
    seam: &mut S,
    cfg: &ConnectConfig,
    pmk: &[u8; 32],
) -> Result<FourWayResult, ConnectError> {
    let mut buf = [0u8; BUF];
    // M1: AP → STA (ANonce).
    let n = seam.rx_eapol(&mut buf)?;
    let m1 = &buf[..n];
    let kf1 = eapol::KeyFrame::parse(m1).ok_or(ConnectError::BadFrame(ConnectEvent::FourWayM1))?;
    let version = kf1.version();
    let mut anonce = [0u8; 32];
    anonce.copy_from_slice(kf1.nonce());
    let mut replay = [0u8; 8];
    replay.copy_from_slice(&m1[9..17]);
    seam.breadcrumb(ConnectEvent::FourWayM1);

    // Derive the PTK from the SAE PMK (the only change vs WPA2 is provenance).
    let mut snonce = [0u8; 32];
    seam.random(&mut snonce);
    let ptk = kdf::ptk_ccmp(pmk, &cfg.ap, &cfg.sta, &anonce, &snonce);

    // M2: STA → AP (SNonce + the WPA3 RSNE + MIC), descriptor v3.
    let mut rsne = [0u8; 24];
    let rsne_len = mlme_logic::build_rsn_ie_wpa3_sae(&mut rsne, 0)
        .ok_or(ConnectError::BadFrame(ConnectEvent::FourWayM2))?;
    let key_info = 3 | eapol::KEY_INFO_KEY_TYPE | eapol::KEY_INFO_MIC; // v3, pairwise
    let mut m2 = [0u8; BUF];
    let m2n = eapol::build_key_frame(
        &mut m2,
        3,
        key_info,
        16,
        &replay,
        &snonce,
        &rsne[..rsne_len],
    )
    .ok_or(ConnectError::BadFrame(ConnectEvent::FourWayM2))?;
    if !eapol::install_mic(version, &ptk.kck, &mut m2[..m2n]) {
        return Err(ConnectError::MicFail(ConnectEvent::FourWayM2));
    }
    seam.tx_eapol(&m2[..m2n])?;
    seam.breadcrumb(ConnectEvent::FourWayM2);

    // M3: AP → STA (GTK/IGTK + MIC).
    let n = seam.rx_eapol(&mut buf)?;
    let m3 = &buf[..n];
    if !eapol::verify_mic(version, &ptk.kck, m3) {
        return Err(ConnectError::MicFail(ConnectEvent::FourWayM3));
    }
    let kf3 = eapol::KeyFrame::parse(m3).ok_or(ConnectError::BadFrame(ConnectEvent::FourWayM3))?;
    let mut replay3 = [0u8; 8];
    replay3.copy_from_slice(&m3[9..17]);
    seam.breadcrumb(ConnectEvent::FourWayM3);

    // Install the pairwise TK.
    seam.install_pairwise(&ptk.tk)?;

    // Unwrap the M3 Key Data (kept for GTK/IGTK extraction in `install_from_m3`).
    let kd_len = kf3.key_data_len() as usize;
    let kd_start = eapol::KEY_DATA_OFFSET;
    let mut m3_key_data = alloc::vec::Vec::new();
    if kd_len >= 16 && m3.len() >= kd_start + kd_len {
        let mut unwrapped = [0u8; 128];
        if let Some(un) =
            keywrap::aes_unwrap(&m3[kd_start..kd_start + kd_len], &ptk.kek, &mut unwrapped)
        {
            m3_key_data.extend_from_slice(&unwrapped[..un]);
        }
    }

    // M4: STA → AP (MIC, secure).
    let key_info4 = 3 | eapol::KEY_INFO_KEY_TYPE | eapol::KEY_INFO_MIC | eapol::KEY_INFO_SECURE;
    let mut m4 = [0u8; BUF];
    let m4n = eapol::build_key_frame(&mut m4, 3, key_info4, 16, &replay3, &[0u8; 32], &[])
        .ok_or(ConnectError::BadFrame(ConnectEvent::FourWayM4))?;
    if !eapol::install_mic(version, &ptk.kck, &mut m4[..m4n]) {
        return Err(ConnectError::MicFail(ConnectEvent::FourWayM4));
    }
    seam.tx_eapol(&m4[..m4n])?;
    seam.breadcrumb(ConnectEvent::FourWayM4);

    Ok(FourWayResult {
        kck: ptk.kck,
        kek: ptk.kek,
        version,
        m3_key_data,
    })
}

/// Extract + install the GTK (to the group WCID) and the IGTK (software store)
/// from the unwrapped M3 Key Data.
fn install_from_m3<S: ConnectSeam>(
    seam: &mut S,
    key_data: &[u8],
) -> Result<(bool, Option<bip::Igtk>), ConnectError> {
    let mut gtk_installed = false;
    if let Some((gtk, idx)) = eapol::gtk_from_kde(key_data) {
        seam.install_gtk(gtk, idx)?;
        gtk_installed = true;
        seam.breadcrumb(ConnectEvent::GtkInstalled);
    }
    let igtk = bip::find_igtk_kde(key_data);
    if igtk.is_some() {
        seam.breadcrumb(ConnectEvent::IgtkInstalled);
    }
    Ok((gtk_installed, igtk))
}

/// The standalone AP-initiated Group-Key-Handshake (M1/M2, key_info 0x1383).
fn group_key_handshake<S: ConnectSeam>(
    seam: &mut S,
    kck: &[u8; 16],
    kek: &[u8; 16],
    version: eapol::KeyDescriptorVersion,
) -> Result<bool, ConnectError> {
    let mut buf = [0u8; BUF];
    let n = seam.rx_eapol(&mut buf)?;
    let m1 = &buf[..n];
    let kf =
        eapol::KeyFrame::parse(m1).ok_or(ConnectError::BadFrame(ConnectEvent::GroupRekeyed))?;
    if !eapol::verify_gkh_m1(&kf) {
        return Err(ConnectError::BadFrame(ConnectEvent::GroupRekeyed));
    }
    if !eapol::verify_mic(version, kck, m1) {
        return Err(ConnectError::MicFail(ConnectEvent::GroupRekeyed));
    }
    let mut replay = [0u8; 8];
    replay.copy_from_slice(&m1[9..17]);
    // Unwrap + install the refreshed GTK.
    let kd_len = kf.key_data_len() as usize;
    let kd_start = eapol::KEY_DATA_OFFSET;
    if kd_len >= 16 && m1.len() >= kd_start + kd_len {
        let mut unwrapped = [0u8; 128];
        if let Some(un) = keywrap::aes_unwrap(&m1[kd_start..kd_start + kd_len], kek, &mut unwrapped)
            && let Some((gtk, idx)) = eapol::gtk_from_kde(&unwrapped[..un])
        {
            seam.install_gtk(gtk, idx)?;
        }
    }
    // M2 ACK.
    let mut m2 = [0u8; BUF];
    let m2n = eapol::build_gkh_m2(&mut m2, 3, &replay)
        .ok_or(ConnectError::BadFrame(ConnectEvent::GroupRekeyed))?;
    if !eapol::install_mic(version, kck, &mut m2[..m2n]) {
        return Err(ConnectError::MicFail(ConnectEvent::GroupRekeyed));
    }
    seam.tx_eapol(&m2[..m2n])?;
    seam.breadcrumb(ConnectEvent::GroupRekeyed);
    Ok(true)
}

/// Verify the headline PMF property: a received forged **unprotected** deauth is
/// ignored (dropped) now that the IGTK is installed (T-058).
fn bip_deauth_check<S: ConnectSeam>(
    seam: &mut S,
    rx: &mut [u8],
    igtk: &Option<bip::Igtk>,
) -> Result<bool, ConnectError> {
    let n = seam.rx_mgmt(rx)?;
    if n < 2 {
        return Err(ConnectError::BadFrame(ConnectEvent::ForgedDeauthDropped));
    }
    let fc = u16::from_le_bytes([rx[0], rx[1]]);
    let has_mmie = match igtk {
        Some(k) => {
            let mut k = k.clone();
            bip::verify_mmie(&mut k, &rx[..n]).is_ok()
        }
        None => false,
    };
    match bip::mgmt_action(fc, true, igtk.is_some(), has_mmie) {
        bip::MgmtAction::DropUnprotected => {
            seam.breadcrumb(ConnectEvent::ForgedDeauthDropped);
            Ok(true)
        }
        bip::MgmtAction::ProcessProtected => {
            seam.breadcrumb(ConnectEvent::ProtectedDeauthAccepted);
            Ok(false)
        }
        _ => Ok(false),
    }
}

// ── small helpers ────────────────────────────────────────────────────────────

/// Parse the fixed `(transaction, status)` of a received SAE Authentication
/// frame (alg is validated by `parse_auth`).
fn parse_auth_fixed(frame: &[u8]) -> Option<(u16, u16)> {
    let a = mlme_logic::parse_auth(frame)?;
    Some((a.transaction, a.status))
}

/// Build an Association Request carrying the WPA3-SAE RSNE (ISC-2).
fn build_assoc_req_sae(buf: &mut [u8], sta: &[u8; 6], ap: &[u8; 6], ssid: &[u8]) -> Option<usize> {
    // Reuse the assoc-req builder for the header + SSID + rates, then swap the
    // RSNE for the WPA3 one. Basic supported rates (1/2/5.5/11 Mbps, basic).
    let rates = [0x82u8, 0x84, 0x8b, 0x96];
    let params = mlme_logic::AssocReqParams {
        ssid,
        rates: &rates,
        capab: mlme_logic::CAP_ESS | mlme_logic::CAP_PRIVACY,
        listen_interval: 1,
        rsn: false, // append the WPA3 RSNE ourselves below
    };
    let n = mlme_logic::build_assoc_req(buf, sta, ap, &params)?;
    mlme_logic::build_rsn_ie_wpa3_sae(buf, n)
}
