// SPDX-License-Identifier: MPL-2.0
//! Shared Atrium client helpers (the client side of `atrium_protocol`).
//!
//! DS-1 `OP_YIELD_FOCUS`: a client hands the keyboard back to the window
//! manager's Home. It is CLIENT-COOPERATIVE: a wedged or malicious client can
//! simply never yield, and so can trap focus until DI-1's kernel
//! secure-attention key lands. That limitation is deliberate and recorded
//! (docs/THINK-2026-10-18-ds1-home-spine-shell.md).
//!
//! DS-1b `OP_LAUNCH` (feature `atrium-launch`): ask the session broker to
//! launch an allowlisted module with a LaunchCap. ADVISORY until the kernel
//! gates `sys_spawn` (OT-66).
//!
//! A9-D `OP_BUSY` (feature `atrium-busy`): reserve a rect of the client's
//! surface for the WM's busy emblem, which turns there on the WM's clock until
//! the client's next commit.

#[cfg(feature = "atrium-busy")]
use crate::atrium_protocol::{Busy, OP_BUSY};
use crate::atrium_protocol::{
    Identity, OP_YIELD_FOCUS, REPLY_LEN, Request, YieldFocus, decode_reply, encode_request,
};
#[cfg(feature = "atrium-launch")]
use crate::atrium_protocol::{LaunchRequest, OP_LAUNCH};
use crate::{Handle, sys_recv, sys_send};

/// Why a request did not complete.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestError {
    /// The request could not be sent on the surface channel.
    Send,
    /// No well-formed reply to this request arrived.
    Reply,
    /// The WM refused it with this protocol status.
    Refused(u32),
}

/// The yield-specific name kept for its callers.
pub type YieldError = RequestError;

/// Send `request` on the surface channel and wait for its reply.
fn call(
    surface: Handle,
    request_id: u64,
    request: Request,
    opcode: u16,
) -> Result<(), RequestError> {
    let (bytes, len) = encode_request(request_id, request);
    if sys_send(surface, &bytes[..len]) != len as i64 {
        return Err(RequestError::Send);
    }
    recv_reply(surface, request_id, opcode)
}

/// Wait for the reply to `request_id` on the surface channel.
fn recv_reply(surface: Handle, request_id: u64, opcode: u16) -> Result<(), RequestError> {
    let mut reply = [0u8; REPLY_LEN];
    if sys_recv(surface, &mut reply) != REPLY_LEN as i64 {
        return Err(RequestError::Reply);
    }
    match decode_reply(&reply) {
        Ok(r) if r.opcode == opcode && r.request_id == request_id => match r.status {
            0 => Ok(()),
            status => Err(RequestError::Refused(status)),
        },
        _ => Err(RequestError::Reply),
    }
}

/// Send `OP_YIELD_FOCUS` for `identity` on its own surface channel and wait
/// for the WM's reply. `Ok(())` iff the WM accepted (status 0).
pub fn yield_focus(
    surface: Handle,
    request_id: u64,
    identity: Identity,
    yield_serial: u64,
) -> Result<(), YieldError> {
    call(
        surface,
        request_id,
        Request::Yield(YieldFocus {
            identity,
            yield_serial,
        }),
        OP_YIELD_FOCUS,
    )
}

/// Ask the broker to launch `module_id`: send `OP_LAUNCH` carrying `token`
/// on the surface channel, then `proof` on the LaunchCap endpoint (honest
/// callers pass `Some(token)`; `None` withholds it — the broker then refuses
/// once its proof deadline passes), then wait for the broker's reply.
/// `Ok(())` iff the broker launched it.
#[cfg(feature = "atrium-launch")]
pub fn launch(
    surface: Handle,
    launch_cap: Handle,
    request_id: u64,
    identity: Identity,
    module_id: u32,
    token: u64,
    proof: Option<u64>,
) -> Result<(), RequestError> {
    let (bytes, len) = encode_request(
        request_id,
        Request::Launch(LaunchRequest {
            identity,
            module_id,
            launch_token: token,
        }),
    );
    if sys_send(surface, &bytes[..len]) != len as i64 {
        return Err(RequestError::Send);
    }
    // A proof that cannot be delivered (the broker closed the cap: it was
    // late) is not an error here: the broker has refused, and its reply must
    // still be received, or the broker would wait in the reply's rendezvous.
    if let Some(proof) = proof {
        let _ = sys_send(launch_cap, &proof.to_le_bytes());
    }
    recv_reply(surface, request_id, OP_LAUNCH)
}

/// Send `OP_BUSY` reserving `rect` (x, y, w, h in the surface's own
/// coordinates) and wait for the WM's reply. `Ok(())` iff the WM accepted;
/// the emblem then shows until this surface's next commit.
#[cfg(feature = "atrium-busy")]
pub fn busy(
    surface: Handle,
    request_id: u64,
    identity: Identity,
    busy_serial: u64,
    rect: (u16, u16, u16, u16),
) -> Result<(), RequestError> {
    let (x, y, width, height) = rect;
    call(
        surface,
        request_id,
        Request::Busy(Busy {
            identity,
            busy_serial,
            x,
            y,
            width,
            height,
        }),
        OP_BUSY,
    )
}
