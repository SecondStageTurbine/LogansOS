// SPDX-License-Identifier: MPL-2.0
//! Atrium G1 surface protocol v1.
//!
//! A `SurfaceCap` is an IPC channel endpoint. The opaque IDs below are lookup
//! keys only; authority comes from the channel on which the record arrived.
//! Every verb repeats the session namespace and the channel provenance token
//! so a future multi-principal compositor can retain the wire ABI unchanged.

pub const MAGIC: u32 = 0x3143_4d4e; // little-endian bytes "NMC1"
pub const VERSION: u16 = 1;
pub const HEADER_LEN: usize = 24;
pub const MAX_MESSAGE_LEN: usize = 72;
pub const REPLY_LEN: usize = 56;
pub const REPLY_PAYLOAD_LEN: usize = 32;
pub const REPLY_OPCODE_BIT: u16 = 0x8000;
pub const MAX_SURFACES: usize = 8;

pub const OP_CREATE_SURFACE: u16 = 1;
pub const OP_IMPORT_ATTACH_BUFFER: u16 = 2;
pub const OP_COMMIT: u16 = 3;
pub const OP_FOCUS_REQUEST: u16 = 4;
pub const OP_DESTROY_SURFACE: u16 = 5;
#[cfg(any(feature = "dw2-scene-policy", test))]
pub const OP_CONFIGURE: u16 = 6;
/// DS-1: the client hands the keyboard back to the WM's Home (Esc). It names
/// only the caller's own surface, on that surface's own channel (INV-WM-1).
#[cfg(any(feature = "atrium-yield", test))]
pub const OP_YIELD_FOCUS: u16 = 7;
/// DS-1b: ask the session broker to launch an allowlisted module, proving
/// possession of a LaunchCap by sending the same token on it. ADVISORY until
/// the kernel gates `sys_spawn` (OT-66).
#[cfg(any(feature = "atrium-launch", test))]
pub const OP_LAUNCH: u16 = 8;
/// S2-F2 fix-forward: a commit that also names the rectangle it changed, in
/// the committing surface's own coordinates, so a compositor that copies the
/// client's pixels (the desktop WM) copies and presents only that band.
/// `OP_COMMIT` is unchanged and still means "the whole surface".
#[cfg(any(feature = "atrium-damage", test))]
pub const OP_COMMIT_DAMAGE: u16 = 9;
/// A9-D: the client is busy (Arthur, blocked in a generation) and reserves a
/// rectangle of its own surface; the compositor animates its busy emblem
/// there on its own clock until the surface's next commit, which ends it.
#[cfg(any(feature = "atrium-busy", test))]
pub const OP_BUSY: u16 = 10;
/// OT-88 S1: set one setting (`settings-core`'s key table). The WM is the
/// store's only writer; it checks the key and value, applies the setting
/// live and replies. A bad one is refused in the reply's status
/// ([`STATUS_SETTING_KEY`], [`STATUS_SETTING_VALUE`]) and the session goes
/// on.
#[cfg(any(feature = "atrium-settings", test))]
pub const OP_SETTING: u16 = 11;
/// `OP_SETTING`'s refusals: no such key; a value outside the key's range;
/// the store couldn't be written (nothing applied); a key only the WM sets
/// (the idle lock); too many writes too fast (the WM's rate limit). A
/// client that isn't the settings app is refused with
/// `ProtocolError::CapRequired`'s status (7) before any of these.
#[cfg(any(feature = "atrium-settings", test))]
pub const STATUS_SETTING_KEY: u32 = 20;
#[cfg(any(feature = "atrium-settings", test))]
pub const STATUS_SETTING_VALUE: u32 = 21;
#[cfg(any(feature = "atrium-settings", test))]
pub const STATUS_SETTING_STORE: u32 = 22;
#[cfg(any(feature = "atrium-settings", test))]
pub const STATUS_SETTING_WM_ONLY: u32 = 23;
#[cfg(any(feature = "atrium-settings", test))]
pub const STATUS_SETTING_BUSY: u32 = 24;
/// L1.3 (THINK-2026-10-30-l1.3 D3): Arthur asks the WM for one action on one
/// path it holds no grant for. The path follows as one raw message on the
/// same channel ([`PERMISSION_PATH_MAX`] bytes at most). The WM shows its own
/// prompt and replies; on [`STATUS_PERMISSION_GRANTED`] it then lends the
/// cap over the same channel (offer `O`, ready `R`, the grant, ack `A`).
#[cfg(any(feature = "atrium-permission", test))]
pub const OP_PERMISSION: u16 = 12;
/// L1.3: the client is done with the action a prompt allowed (the outcome:
/// 0 or a negative errno, as its magnitude). The WM revokes an "Allow once"
/// cap here.
#[cfg(any(feature = "atrium-permission", test))]
pub const OP_PERMISSION_DONE: u16 = 13;
/// `OP_PERMISSION`'s answers: allowed once (a cap follows; the WM revokes it
/// at `OP_PERMISSION_DONE`), allowed always (a cap follows and stays for the
/// session; the grant is recorded), denied (by the user, or remembered for
/// the session), refused before any prompt because the session is read-only
/// and the action writes, and a path the WM won't prompt for (not under
/// `/neofs`, or under `.system/`).
#[cfg(any(feature = "atrium-permission", test))]
pub const STATUS_PERMISSION_GRANTED: u32 = 0;
#[cfg(any(feature = "atrium-permission", test))]
pub const STATUS_PERMISSION_GRANTED_ALWAYS: u32 = 30;
#[cfg(any(feature = "atrium-permission", test))]
pub const STATUS_PERMISSION_DENIED: u32 = 31;
#[cfg(any(feature = "atrium-permission", test))]
pub const STATUS_PERMISSION_READ_ONLY: u32 = 32;
#[cfg(any(feature = "atrium-permission", test))]
pub const STATUS_PERMISSION_PATH: u32 = 33;
/// The longest path an `OP_PERMISSION` names.
#[cfg(any(feature = "atrium-permission", test))]
pub const PERMISSION_PATH_MAX: usize = 255;
/// The action an `OP_PERMISSION` asks for (A1/A2 only; D6).
#[cfg(any(feature = "atrium-permission", test))]
pub const PERMISSION_READ: u16 = 1;
#[cfg(any(feature = "atrium-permission", test))]
pub const PERMISSION_LIST: u16 = 2;
#[cfg(any(feature = "atrium-permission", test))]
pub const PERMISSION_WRITE: u16 = 3;

/// Whether `opcode` is a request verb compiled into this build.
const fn is_request_opcode(opcode: u16) -> bool {
    match opcode {
        OP_CREATE_SURFACE
        | OP_IMPORT_ATTACH_BUFFER
        | OP_COMMIT
        | OP_FOCUS_REQUEST
        | OP_DESTROY_SURFACE => true,
        #[cfg(any(feature = "dw2-scene-policy", test))]
        OP_CONFIGURE => true,
        #[cfg(any(feature = "atrium-yield", test))]
        OP_YIELD_FOCUS => true,
        #[cfg(any(feature = "atrium-launch", test))]
        OP_LAUNCH => true,
        #[cfg(any(feature = "atrium-damage", test))]
        OP_COMMIT_DAMAGE => true,
        #[cfg(any(feature = "atrium-busy", test))]
        OP_BUSY => true,
        #[cfg(any(feature = "atrium-settings", test))]
        OP_SETTING => true,
        #[cfg(any(feature = "atrium-permission", test))]
        OP_PERMISSION | OP_PERMISSION_DONE => true,
        _ => false,
    }
}

pub const CREATE_PAYLOAD_LEN: usize = 48;
pub const IMPORT_ATTACH_PAYLOAD_LEN: usize = 40;
pub const COMMIT_PAYLOAD_LEN: usize = 32;
pub const FOCUS_PAYLOAD_LEN: usize = 40;
pub const DESTROY_PAYLOAD_LEN: usize = 32;
#[cfg(any(feature = "dw2-scene-policy", test))]
pub const CONFIGURE_PAYLOAD_LEN: usize = 40;
#[cfg(any(feature = "atrium-yield", test))]
pub const YIELD_PAYLOAD_LEN: usize = 32;
#[cfg(any(feature = "atrium-launch", test))]
pub const LAUNCH_PAYLOAD_LEN: usize = 40;
#[cfg(any(feature = "atrium-damage", test))]
pub const COMMIT_DAMAGE_PAYLOAD_LEN: usize = 40;
#[cfg(any(feature = "atrium-busy", test))]
pub const BUSY_PAYLOAD_LEN: usize = 40;
#[cfg(any(feature = "atrium-settings", test))]
pub const SETTING_PAYLOAD_LEN: usize = 40;
#[cfg(any(feature = "atrium-permission", test))]
pub const PERMISSION_PAYLOAD_LEN: usize = 40;
#[cfg(any(feature = "atrium-permission", test))]
pub const PERMISSION_DONE_PAYLOAD_LEN: usize = 40;

pub const ROLE_WORKFIELD_ROOT: u32 = 1;
pub const ROLE_TEXT: u32 = 2;
pub const FOCUS_REASON_USER_ACTIVATION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtocolError {
    BadVersion,
    BadLength,
    BadField,
    BadState,
    StaleObject,
    WrongOwner,
    CapRequired,
    Limit,
    SurfaceGone,
}

impl ProtocolError {
    pub const fn status(self) -> u32 {
        match self {
            Self::BadVersion => 1,
            Self::BadLength => 2,
            Self::BadField => 3,
            Self::BadState => 4,
            Self::StaleObject => 5,
            Self::WrongOwner => 6,
            Self::CapRequired => 7,
            Self::Limit => 8,
            Self::SurfaceGone => 9,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Header {
    pub opcode: u16,
    pub payload_len: u32,
    pub request_id: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reply {
    pub opcode: u16,
    pub request_id: u64,
    /// Zero is success; non-zero values are the stable protocol-error domain.
    pub status: u32,
    pub identity: Identity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identity {
    pub namespace_id: u64,
    pub surface_id: u64,
    /// Provenance of the dedicated channel endpoint carrying this verb.
    pub surface_cap_provenance: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CreateSurface {
    pub identity: Identity,
    pub role: u32,
    pub width: u32,
    pub height: u32,
    pub client_tag: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImportAttachBuffer {
    pub identity: Identity,
    pub transfer_id: u64,
    /// Provenance of the BufferCap offered by `sys_grant` on this channel.
    pub buffer_cap_provenance: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Commit {
    pub identity: Identity,
    pub client_serial: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FocusRequest {
    pub identity: Identity,
    pub reason: u32,
    pub request_serial: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DestroySurface {
    pub identity: Identity,
    pub final_client_serial: u64,
}

#[cfg(any(feature = "dw2-scene-policy", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ConfigureSurface {
    pub identity: Identity,
    pub width: u32,
    pub height: u32,
    pub configure_serial: u64,
}

/// DS-1 `OP_YIELD_FOCUS`: identity + a non-zero client serial.
#[cfg(any(feature = "atrium-yield", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct YieldFocus {
    pub identity: Identity,
    pub yield_serial: u64,
}

/// DS-1b `OP_LAUNCH`: identity, the module id (enumerated, never a name),
/// a zero reserved word, and the token (non-zero) the client also sends on
/// its LaunchCap endpoint.
#[cfg(any(feature = "atrium-launch", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LaunchRequest {
    pub identity: Identity,
    pub module_id: u32,
    pub launch_token: u64,
}

/// `OP_COMMIT_DAMAGE`: `Commit` plus the changed rectangle (surface
/// coordinates, non-empty). Bytes 48..56 the serial, then x, y, w, h as
/// little-endian u16 at 56, 58, 60, 62.
#[cfg(any(feature = "atrium-damage", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommitDamage {
    pub identity: Identity,
    pub client_serial: u64,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

/// A9-D `OP_BUSY`: identity, a non-zero client serial, and the reserved
/// rectangle (surface coordinates, non-empty), laid out as `CommitDamage`:
/// the serial at 48..56, then x, y, w, h as little-endian u16 at 56..64.
#[cfg(any(feature = "atrium-busy", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Busy {
    pub identity: Identity,
    pub busy_serial: u64,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

/// OT-88 S1 `OP_SETTING`: identity, the key id (`u16` at 48), a zero
/// reserved `u16` at 50, the value (`u32` at 52) and a zero reserved `u64`
/// at 56. The key and value are the WM's to check, against
/// `settings-core`'s table; the decoder checks only the reserved words.
#[cfg(any(feature = "atrium-settings", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Setting {
    pub identity: Identity,
    pub key: u16,
    pub value: u32,
}

/// L1.3 `OP_PERMISSION`: identity, the action (`u16` at 48: read, list,
/// write), the path's length (`u16` at 50, 1..=[`PERMISSION_PATH_MAX`]),
/// the intent's length (`u16` at 52, 0..=[`PERMISSION_PATH_MAX`]; L2.1: the
/// line the action came from, sent after the path; 0: none), and zero
/// reserved words (`u16` at 54, `u64` at 56).
#[cfg(any(feature = "atrium-permission", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Permission {
    pub identity: Identity,
    pub action: u16,
    pub path_len: u16,
    pub intent_len: u16,
}

/// L1.3 `OP_PERMISSION_DONE`: identity, the outcome (`u32` at 48: 0, or a
/// failed action's errno magnitude), and zero reserved words (`u32` at 52,
/// `u64` at 56).
#[cfg(any(feature = "atrium-permission", test))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PermissionDone {
    pub identity: Identity,
    pub outcome: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Request {
    Create(CreateSurface),
    ImportAttach(ImportAttachBuffer),
    Commit(Commit),
    Focus(FocusRequest),
    Destroy(DestroySurface),
    #[cfg(any(feature = "dw2-scene-policy", test))]
    Configure(ConfigureSurface),
    #[cfg(any(feature = "atrium-yield", test))]
    Yield(YieldFocus),
    #[cfg(any(feature = "atrium-launch", test))]
    Launch(LaunchRequest),
    #[cfg(any(feature = "atrium-damage", test))]
    CommitDamage(CommitDamage),
    #[cfg(any(feature = "atrium-busy", test))]
    Busy(Busy),
    #[cfg(any(feature = "atrium-settings", test))]
    Setting(Setting),
    #[cfg(any(feature = "atrium-permission", test))]
    Permission(Permission),
    #[cfg(any(feature = "atrium-permission", test))]
    PermissionDone(PermissionDone),
}

fn put_u16(out: &mut [u8], at: usize, value: u16) {
    out[at..at + 2].copy_from_slice(&value.to_le_bytes());
}
fn put_u32(out: &mut [u8], at: usize, value: u32) {
    out[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn put_u64(out: &mut [u8], at: usize, value: u64) {
    out[at..at + 8].copy_from_slice(&value.to_le_bytes());
}
fn get_u16(input: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([input[at], input[at + 1]])
}
fn get_u32(input: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([input[at], input[at + 1], input[at + 2], input[at + 3]])
}
fn get_u64(input: &[u8], at: usize) -> u64 {
    u64::from_le_bytes([
        input[at],
        input[at + 1],
        input[at + 2],
        input[at + 3],
        input[at + 4],
        input[at + 5],
        input[at + 6],
        input[at + 7],
    ])
}

fn encode_header(out: &mut [u8], opcode: u16, payload_len: usize, request_id: u64) {
    put_u32(out, 0, MAGIC);
    put_u16(out, 4, VERSION);
    put_u16(out, 6, opcode);
    put_u32(out, 8, payload_len as u32);
    put_u32(out, 12, 0);
    put_u64(out, 16, request_id);
}

fn encode_identity(out: &mut [u8], identity: Identity) {
    put_u64(out, 24, identity.namespace_id);
    put_u64(out, 32, identity.surface_id);
    put_u64(out, 40, identity.surface_cap_provenance);
}

pub fn encode_request(request_id: u64, request: Request) -> ([u8; MAX_MESSAGE_LEN], usize) {
    let mut out = [0u8; MAX_MESSAGE_LEN];
    let (opcode, payload_len) = match request {
        Request::Create(v) => {
            encode_identity(&mut out, v.identity);
            put_u32(&mut out, 48, v.role);
            put_u32(&mut out, 52, v.width);
            put_u32(&mut out, 56, v.height);
            put_u32(&mut out, 60, 0);
            put_u64(&mut out, 64, v.client_tag);
            (OP_CREATE_SURFACE, CREATE_PAYLOAD_LEN)
        }
        Request::ImportAttach(v) => {
            encode_identity(&mut out, v.identity);
            put_u64(&mut out, 48, v.transfer_id);
            put_u64(&mut out, 56, v.buffer_cap_provenance);
            (OP_IMPORT_ATTACH_BUFFER, IMPORT_ATTACH_PAYLOAD_LEN)
        }
        Request::Commit(v) => {
            encode_identity(&mut out, v.identity);
            put_u64(&mut out, 48, v.client_serial);
            (OP_COMMIT, COMMIT_PAYLOAD_LEN)
        }
        Request::Focus(v) => {
            encode_identity(&mut out, v.identity);
            put_u32(&mut out, 48, v.reason);
            put_u32(&mut out, 52, 0);
            put_u64(&mut out, 56, v.request_serial);
            (OP_FOCUS_REQUEST, FOCUS_PAYLOAD_LEN)
        }
        Request::Destroy(v) => {
            encode_identity(&mut out, v.identity);
            put_u64(&mut out, 48, v.final_client_serial);
            (OP_DESTROY_SURFACE, DESTROY_PAYLOAD_LEN)
        }
        #[cfg(any(feature = "dw2-scene-policy", test))]
        Request::Configure(v) => {
            encode_identity(&mut out, v.identity);
            put_u32(&mut out, 48, v.width);
            put_u32(&mut out, 52, v.height);
            put_u64(&mut out, 56, v.configure_serial);
            (OP_CONFIGURE, CONFIGURE_PAYLOAD_LEN)
        }
        #[cfg(any(feature = "atrium-yield", test))]
        Request::Yield(v) => {
            encode_identity(&mut out, v.identity);
            put_u64(&mut out, 48, v.yield_serial);
            (OP_YIELD_FOCUS, YIELD_PAYLOAD_LEN)
        }
        #[cfg(any(feature = "atrium-launch", test))]
        Request::Launch(v) => {
            encode_identity(&mut out, v.identity);
            put_u32(&mut out, 48, v.module_id);
            put_u32(&mut out, 52, 0);
            put_u64(&mut out, 56, v.launch_token);
            (OP_LAUNCH, LAUNCH_PAYLOAD_LEN)
        }
        #[cfg(any(feature = "atrium-damage", test))]
        Request::CommitDamage(v) => {
            encode_identity(&mut out, v.identity);
            put_u64(&mut out, 48, v.client_serial);
            put_u16(&mut out, 56, v.x);
            put_u16(&mut out, 58, v.y);
            put_u16(&mut out, 60, v.width);
            put_u16(&mut out, 62, v.height);
            (OP_COMMIT_DAMAGE, COMMIT_DAMAGE_PAYLOAD_LEN)
        }
        #[cfg(any(feature = "atrium-busy", test))]
        Request::Busy(v) => {
            encode_identity(&mut out, v.identity);
            put_u64(&mut out, 48, v.busy_serial);
            put_u16(&mut out, 56, v.x);
            put_u16(&mut out, 58, v.y);
            put_u16(&mut out, 60, v.width);
            put_u16(&mut out, 62, v.height);
            (OP_BUSY, BUSY_PAYLOAD_LEN)
        }
        #[cfg(any(feature = "atrium-settings", test))]
        Request::Setting(v) => {
            encode_identity(&mut out, v.identity);
            put_u16(&mut out, 48, v.key);
            put_u32(&mut out, 52, v.value);
            (OP_SETTING, SETTING_PAYLOAD_LEN)
        }
        #[cfg(any(feature = "atrium-permission", test))]
        Request::Permission(v) => {
            encode_identity(&mut out, v.identity);
            put_u16(&mut out, 48, v.action);
            put_u16(&mut out, 50, v.path_len);
            put_u16(&mut out, 52, v.intent_len);
            (OP_PERMISSION, PERMISSION_PAYLOAD_LEN)
        }
        #[cfg(any(feature = "atrium-permission", test))]
        Request::PermissionDone(v) => {
            encode_identity(&mut out, v.identity);
            put_u32(&mut out, 48, v.outcome);
            (OP_PERMISSION_DONE, PERMISSION_DONE_PAYLOAD_LEN)
        }
    };
    encode_header(&mut out, opcode, payload_len, request_id);
    (out, HEADER_LEN + payload_len)
}

pub fn encode_reply(reply: Reply) -> [u8; REPLY_LEN] {
    let mut out = [0u8; REPLY_LEN];
    encode_header(
        &mut out,
        reply.opcode | REPLY_OPCODE_BIT,
        REPLY_PAYLOAD_LEN,
        reply.request_id,
    );
    put_u32(&mut out, 24, reply.status);
    put_u32(&mut out, 28, 0);
    put_u64(&mut out, 32, reply.identity.namespace_id);
    put_u64(&mut out, 40, reply.identity.surface_id);
    put_u64(&mut out, 48, reply.identity.surface_cap_provenance);
    out
}

/// F11 S5: an `OP_CREATE_SURFACE` reply that hands the client the session's
/// keyboard layout id (`keymap_layout_logic.rs`; 0: US) in the reply's
/// reserved word at byte 28, so a client that decodes text types in the
/// layout the WM decodes in. The id US is the plain [`encode_reply`], byte
/// for byte. Only a client that asks for it gets it: [`decode_reply`] still
/// refuses a non-zero word, so the WM sends it only to such a client.
#[cfg(any(feature = "atrium-keymap", test))]
pub fn encode_create_reply(reply: Reply, layout: u32) -> [u8; REPLY_LEN] {
    let mut out = encode_reply(Reply {
        opcode: OP_CREATE_SURFACE,
        ..reply
    });
    put_u32(&mut out, 28, layout);
    out
}

/// The client side of [`encode_create_reply`]: the reply and the layout id.
/// Anything but an `OP_CREATE_SURFACE` reply is refused, as [`decode_reply`]
/// refuses it; the id is the client's to range-check.
#[cfg(any(feature = "atrium-keymap", test))]
pub fn decode_create_reply(input: &[u8]) -> Result<(Reply, u32), ProtocolError> {
    if input.len() != REPLY_LEN {
        return Err(ProtocolError::BadLength);
    }
    let mut plain = [0u8; REPLY_LEN];
    plain.copy_from_slice(input);
    let layout = get_u32(input, 28);
    put_u32(&mut plain, 28, 0);
    let reply = decode_reply(&plain)?;
    if reply.opcode != OP_CREATE_SURFACE {
        return Err(ProtocolError::BadField);
    }
    Ok((reply, layout))
}

/// The request a message that failed [`decode_request`] still names, so the
/// receiver can refuse it in a reply: `(opcode, request_id)` when the bytes
/// hold a whole header of this protocol (magic, version) for a request (no
/// reply bit), whatever its opcode, length or payload. `None`: nothing to
/// reply to.
pub fn request_target(input: &[u8]) -> Option<(u16, u64)> {
    if input.len() < HEADER_LEN || get_u32(input, 0) != MAGIC || get_u16(input, 4) != VERSION {
        return None;
    }
    let opcode = get_u16(input, 6);
    (opcode & REPLY_OPCODE_BIT == 0).then(|| (opcode, get_u64(input, 16)))
}

pub fn decode_reply(input: &[u8]) -> Result<Reply, ProtocolError> {
    if input.len() != REPLY_LEN {
        return Err(ProtocolError::BadLength);
    }
    if get_u32(input, 0) != MAGIC || get_u16(input, 4) != VERSION {
        return Err(ProtocolError::BadVersion);
    }
    let wire_opcode = get_u16(input, 6);
    let opcode = wire_opcode & !REPLY_OPCODE_BIT;
    if wire_opcode & REPLY_OPCODE_BIT == 0 || !is_request_opcode(opcode) {
        return Err(ProtocolError::BadVersion);
    }
    if get_u32(input, 8) != REPLY_PAYLOAD_LEN as u32
        || get_u32(input, 12) != 0
        || get_u32(input, 28) != 0
    {
        return Err(ProtocolError::BadField);
    }
    Ok(Reply {
        opcode,
        request_id: get_u64(input, 16),
        status: get_u32(input, 24),
        identity: Identity {
            namespace_id: get_u64(input, 32),
            surface_id: get_u64(input, 40),
            surface_cap_provenance: get_u64(input, 48),
        },
    })
}

pub fn decode_request(input: &[u8]) -> Result<(Header, Request), ProtocolError> {
    if input.len() < HEADER_LEN {
        return Err(ProtocolError::BadLength);
    }
    if get_u32(input, 0) != MAGIC || get_u16(input, 4) != VERSION {
        return Err(ProtocolError::BadVersion);
    }
    if get_u32(input, 12) != 0 {
        return Err(ProtocolError::BadField);
    }
    let opcode = get_u16(input, 6);
    let payload_len = get_u32(input, 8) as usize;
    let expected = match opcode {
        OP_CREATE_SURFACE => CREATE_PAYLOAD_LEN,
        OP_IMPORT_ATTACH_BUFFER => IMPORT_ATTACH_PAYLOAD_LEN,
        OP_COMMIT => COMMIT_PAYLOAD_LEN,
        OP_FOCUS_REQUEST => FOCUS_PAYLOAD_LEN,
        OP_DESTROY_SURFACE => DESTROY_PAYLOAD_LEN,
        #[cfg(any(feature = "dw2-scene-policy", test))]
        OP_CONFIGURE => CONFIGURE_PAYLOAD_LEN,
        #[cfg(any(feature = "atrium-yield", test))]
        OP_YIELD_FOCUS => YIELD_PAYLOAD_LEN,
        #[cfg(any(feature = "atrium-launch", test))]
        OP_LAUNCH => LAUNCH_PAYLOAD_LEN,
        #[cfg(any(feature = "atrium-damage", test))]
        OP_COMMIT_DAMAGE => COMMIT_DAMAGE_PAYLOAD_LEN,
        #[cfg(any(feature = "atrium-busy", test))]
        OP_BUSY => BUSY_PAYLOAD_LEN,
        #[cfg(any(feature = "atrium-settings", test))]
        OP_SETTING => SETTING_PAYLOAD_LEN,
        #[cfg(any(feature = "atrium-permission", test))]
        OP_PERMISSION => PERMISSION_PAYLOAD_LEN,
        #[cfg(any(feature = "atrium-permission", test))]
        OP_PERMISSION_DONE => PERMISSION_DONE_PAYLOAD_LEN,
        _ => return Err(ProtocolError::BadVersion),
    };
    if payload_len != expected || input.len() != HEADER_LEN + expected {
        return Err(ProtocolError::BadLength);
    }
    let identity = Identity {
        namespace_id: get_u64(input, 24),
        surface_id: get_u64(input, 32),
        surface_cap_provenance: get_u64(input, 40),
    };
    let request = match opcode {
        OP_CREATE_SURFACE => {
            if get_u32(input, 60) != 0 {
                return Err(ProtocolError::BadField);
            }
            Request::Create(CreateSurface {
                identity,
                role: get_u32(input, 48),
                width: get_u32(input, 52),
                height: get_u32(input, 56),
                client_tag: get_u64(input, 64),
            })
        }
        OP_IMPORT_ATTACH_BUFFER => Request::ImportAttach(ImportAttachBuffer {
            identity,
            transfer_id: get_u64(input, 48),
            buffer_cap_provenance: get_u64(input, 56),
        }),
        OP_COMMIT => Request::Commit(Commit {
            identity,
            client_serial: get_u64(input, 48),
        }),
        OP_FOCUS_REQUEST => {
            if get_u32(input, 52) != 0 {
                return Err(ProtocolError::BadField);
            }
            Request::Focus(FocusRequest {
                identity,
                reason: get_u32(input, 48),
                request_serial: get_u64(input, 56),
            })
        }
        OP_DESTROY_SURFACE => Request::Destroy(DestroySurface {
            identity,
            final_client_serial: get_u64(input, 48),
        }),
        #[cfg(any(feature = "dw2-scene-policy", test))]
        OP_CONFIGURE => Request::Configure(ConfigureSurface {
            identity,
            width: get_u32(input, 48),
            height: get_u32(input, 52),
            configure_serial: get_u64(input, 56),
        }),
        #[cfg(any(feature = "atrium-yield", test))]
        OP_YIELD_FOCUS => {
            let yield_serial = get_u64(input, 48);
            if yield_serial == 0 {
                return Err(ProtocolError::BadField);
            }
            Request::Yield(YieldFocus {
                identity,
                yield_serial,
            })
        }
        #[cfg(any(feature = "atrium-launch", test))]
        OP_LAUNCH => {
            let module_id = get_u32(input, 48);
            let launch_token = get_u64(input, 56);
            if module_id == 0 || get_u32(input, 52) != 0 || launch_token == 0 {
                return Err(ProtocolError::BadField);
            }
            Request::Launch(LaunchRequest {
                identity,
                module_id,
                launch_token,
            })
        }
        #[cfg(any(feature = "atrium-damage", test))]
        OP_COMMIT_DAMAGE => {
            let (width, height) = (get_u16(input, 60), get_u16(input, 62));
            if width == 0 || height == 0 {
                return Err(ProtocolError::BadField);
            }
            Request::CommitDamage(CommitDamage {
                identity,
                client_serial: get_u64(input, 48),
                x: get_u16(input, 56),
                y: get_u16(input, 58),
                width,
                height,
            })
        }
        #[cfg(any(feature = "atrium-busy", test))]
        OP_BUSY => {
            let busy_serial = get_u64(input, 48);
            let (width, height) = (get_u16(input, 60), get_u16(input, 62));
            if busy_serial == 0 || width == 0 || height == 0 {
                return Err(ProtocolError::BadField);
            }
            Request::Busy(Busy {
                identity,
                busy_serial,
                x: get_u16(input, 56),
                y: get_u16(input, 58),
                width,
                height,
            })
        }
        #[cfg(any(feature = "atrium-settings", test))]
        OP_SETTING => {
            if get_u16(input, 50) != 0 || get_u64(input, 56) != 0 {
                return Err(ProtocolError::BadField);
            }
            Request::Setting(Setting {
                identity,
                key: get_u16(input, 48),
                value: get_u32(input, 52),
            })
        }
        #[cfg(any(feature = "atrium-permission", test))]
        OP_PERMISSION => {
            let (action, path_len) = (get_u16(input, 48), get_u16(input, 50));
            let intent_len = get_u16(input, 52);
            if !(PERMISSION_READ..=PERMISSION_WRITE).contains(&action)
                || path_len == 0
                || usize::from(path_len) > PERMISSION_PATH_MAX
                || usize::from(intent_len) > PERMISSION_PATH_MAX
                || get_u16(input, 54) != 0
                || get_u64(input, 56) != 0
            {
                return Err(ProtocolError::BadField);
            }
            Request::Permission(Permission {
                identity,
                action,
                path_len,
                intent_len,
            })
        }
        #[cfg(any(feature = "atrium-permission", test))]
        OP_PERMISSION_DONE => {
            if get_u32(input, 52) != 0 || get_u64(input, 56) != 0 {
                return Err(ProtocolError::BadField);
            }
            Request::PermissionDone(PermissionDone {
                identity,
                outcome: get_u32(input, 48),
            })
        }
        _ => unreachable!(),
    };
    Ok((
        Header {
            opcode,
            payload_len: payload_len as u32,
            request_id: get_u64(input, 16),
        },
        request,
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Surface {
    id: u64,
    cap_provenance: u64,
    role: u32,
    attached_buffer_provenance: u64,
    committed_serial: u64,
    #[cfg(any(feature = "dw2-scene-policy", test))]
    configured_width: u32,
    #[cfg(any(feature = "dw2-scene-policy", test))]
    configured_height: u32,
    /// The size the surface was created with (the bound for its damage).
    #[cfg(any(feature = "atrium-damage", test))]
    created_width: u32,
    #[cfg(any(feature = "atrium-damage", test))]
    created_height: u32,
    alive: bool,
}

const EMPTY_SURFACE: Surface = Surface {
    id: 0,
    cap_provenance: 0,
    role: 0,
    attached_buffer_provenance: 0,
    committed_serial: 0,
    #[cfg(any(feature = "dw2-scene-policy", test))]
    configured_width: 0,
    #[cfg(any(feature = "dw2-scene-policy", test))]
    configured_height: 0,
    #[cfg(any(feature = "atrium-damage", test))]
    created_width: 0,
    #[cfg(any(feature = "atrium-damage", test))]
    created_height: 0,
    alive: false,
};

/// Bounded, userland compositor registry. Dispatch does fixed-array scans and
/// never takes an AI-governance lock; one message performs O(MAX_SURFACES)
/// work, so a noisy client cannot create an unbounded compositor stall.
pub struct SurfaceRegistry {
    namespace_id: u64,
    output_width: u32,
    output_height: u32,
    next_surface_id: u64,
    surfaces: [Surface; MAX_SURFACES],
}

impl SurfaceRegistry {
    pub const fn new(namespace_id: u64, output_width: u32, output_height: u32) -> Self {
        Self {
            namespace_id,
            output_width,
            output_height,
            next_surface_id: 1,
            surfaces: [EMPTY_SURFACE; MAX_SURFACES],
        }
    }

    pub fn create(
        &mut self,
        request: CreateSurface,
        cap_provenance: u64,
    ) -> Result<Identity, ProtocolError> {
        if request.identity.namespace_id != self.namespace_id || request.identity.surface_id != 0 {
            return Err(ProtocolError::WrongOwner);
        }
        if cap_provenance == 0 || request.identity.surface_cap_provenance != 0 {
            return Err(ProtocolError::CapRequired);
        }
        match request.role {
            ROLE_WORKFIELD_ROOT => {
                if request.width != self.output_width || request.height != self.output_height {
                    return Err(ProtocolError::BadField);
                }
                if self
                    .surfaces
                    .iter()
                    .any(|s| s.alive && s.role == ROLE_WORKFIELD_ROOT)
                {
                    return Err(ProtocolError::Limit);
                }
            }
            ROLE_TEXT => {
                if request.width == 0
                    || request.height == 0
                    || request.width > self.output_width
                    || request.height > self.output_height
                {
                    return Err(ProtocolError::BadField);
                }
            }
            _ => return Err(ProtocolError::BadField),
        }
        let slot = self
            .surfaces
            .iter_mut()
            .find(|s| !s.alive)
            .ok_or(ProtocolError::Limit)?;
        let id = self.next_surface_id;
        self.next_surface_id = self
            .next_surface_id
            .checked_add(1)
            .ok_or(ProtocolError::Limit)?;
        *slot = Surface {
            id,
            cap_provenance,
            role: request.role,
            attached_buffer_provenance: 0,
            committed_serial: 0,
            #[cfg(any(feature = "dw2-scene-policy", test))]
            configured_width: request.width,
            #[cfg(any(feature = "dw2-scene-policy", test))]
            configured_height: request.height,
            #[cfg(any(feature = "atrium-damage", test))]
            created_width: request.width,
            #[cfg(any(feature = "atrium-damage", test))]
            created_height: request.height,
            alive: true,
        };
        Ok(Identity {
            namespace_id: self.namespace_id,
            surface_id: id,
            surface_cap_provenance: cap_provenance,
        })
    }

    fn lookup_mut(&mut self, identity: Identity) -> Result<&mut Surface, ProtocolError> {
        if identity.namespace_id != self.namespace_id {
            return Err(ProtocolError::WrongOwner);
        }
        let surface = self
            .surfaces
            .iter_mut()
            .find(|s| s.id == identity.surface_id)
            .ok_or(ProtocolError::StaleObject)?;
        if !surface.alive {
            return Err(ProtocolError::SurfaceGone);
        }
        if surface.cap_provenance != identity.surface_cap_provenance {
            return Err(ProtocolError::CapRequired);
        }
        Ok(surface)
    }

    fn lookup_mut_on_channel(
        &mut self,
        identity: Identity,
        received_cap_provenance: u64,
    ) -> Result<&mut Surface, ProtocolError> {
        if received_cap_provenance == 0
            || identity.surface_cap_provenance != received_cap_provenance
        {
            return Err(ProtocolError::CapRequired);
        }
        self.lookup_mut(identity)
    }

    pub fn import_attach_on_channel(
        &mut self,
        request: ImportAttachBuffer,
        received_surface_cap_provenance: u64,
        received_buffer_cap_provenance: u64,
    ) -> Result<(), ProtocolError> {
        if request.transfer_id == 0
            || request.buffer_cap_provenance == 0
            || request.buffer_cap_provenance != received_buffer_cap_provenance
        {
            return Err(ProtocolError::CapRequired);
        }
        self.lookup_mut_on_channel(request.identity, received_surface_cap_provenance)?
            .attached_buffer_provenance = received_buffer_cap_provenance;
        Ok(())
    }

    pub fn commit_on_channel(
        &mut self,
        request: Commit,
        received_cap_provenance: u64,
    ) -> Result<u64, ProtocolError> {
        let surface = self.lookup_mut_on_channel(request.identity, received_cap_provenance)?;
        Self::commit_surface(surface, request.client_serial)
    }

    /// S2-F2: a commit naming its damage, honoured only on the surface's own
    /// channel (INV-WM-1, exactly as `commit_on_channel`): the rect must be
    /// inside the surface as created, else `BadField` and nothing commits.
    /// Returns the serial and the (validated) rect.
    #[cfg(any(feature = "atrium-damage", test))]
    pub fn commit_damage_on_channel(
        &mut self,
        request: CommitDamage,
        received_cap_provenance: u64,
    ) -> Result<(u64, CommitDamage), ProtocolError> {
        let surface = self.lookup_mut_on_channel(request.identity, received_cap_provenance)?;
        let right = u32::from(request.x) + u32::from(request.width);
        let bottom = u32::from(request.y) + u32::from(request.height);
        let (bound_w, bound_h) = Self::damage_bound(surface);
        if right > bound_w || bottom > bound_h {
            return Err(ProtocolError::BadField);
        }
        let serial = Self::commit_surface(surface, request.client_serial)?;
        Ok((serial, request))
    }

    /// A9-D: a busy rect is honoured only on the surface's own channel and
    /// only inside the surface (the same bound as a damage rect); it commits
    /// nothing. Returns the validated request.
    #[cfg(any(feature = "atrium-busy", test))]
    pub fn busy_on_channel(
        &mut self,
        request: Busy,
        received_cap_provenance: u64,
    ) -> Result<Busy, ProtocolError> {
        let surface = self.lookup_mut_on_channel(request.identity, received_cap_provenance)?;
        let right = u32::from(request.x) + u32::from(request.width);
        let bottom = u32::from(request.y) + u32::from(request.height);
        let (bound_w, bound_h) = Self::damage_bound(surface);
        if right > bound_w || bottom > bound_h {
            return Err(ProtocolError::BadField);
        }
        Ok(request)
    }

    /// What a damage rect is bounded by: the size the surface was created
    /// with, or (DI-1b resize, with the configure verb compiled in) the size
    /// it last acknowledged through [`Self::configure_on_channel`].
    #[cfg(any(feature = "atrium-damage", test))]
    fn damage_bound(surface: &Surface) -> (u32, u32) {
        #[cfg(any(feature = "dw2-scene-policy", test))]
        if surface.configured_width != 0 {
            return (surface.configured_width, surface.configured_height);
        }
        (surface.created_width, surface.created_height)
    }

    fn commit_surface(surface: &mut Surface, client_serial: u64) -> Result<u64, ProtocolError> {
        if surface.attached_buffer_provenance == 0 || client_serial <= surface.committed_serial {
            return Err(ProtocolError::BadState);
        }
        surface.committed_serial = client_serial;
        Ok(client_serial)
    }

    pub fn focus_request_on_channel(
        &mut self,
        request: FocusRequest,
        received_cap_provenance: u64,
        policy: impl FnOnce(u64, u32) -> bool,
    ) -> Result<bool, ProtocolError> {
        if request.reason != FOCUS_REASON_USER_ACTIVATION || request.request_serial == 0 {
            return Err(ProtocolError::BadField);
        }
        let surface = self.lookup_mut_on_channel(request.identity, received_cap_provenance)?;
        Ok(policy(surface.id, surface.role))
    }

    /// DS-1: a yield is honoured only for a live surface named on its own
    /// channel; the WM then takes the keyboard back for Home. Returns the
    /// surface id.
    #[cfg(any(feature = "atrium-yield", test))]
    pub fn yield_focus_on_channel(
        &mut self,
        request: YieldFocus,
        received_cap_provenance: u64,
    ) -> Result<u64, ProtocolError> {
        let surface = self.lookup_mut_on_channel(request.identity, received_cap_provenance)?;
        Ok(surface.id)
    }

    pub fn destroy_on_channel(
        &mut self,
        request: DestroySurface,
        received_cap_provenance: u64,
    ) -> Result<(), ProtocolError> {
        let surface = self.lookup_mut_on_channel(request.identity, received_cap_provenance)?;
        Self::destroy_surface(surface, request.final_client_serial)
    }

    fn destroy_surface(
        surface: &mut Surface,
        final_client_serial: u64,
    ) -> Result<(), ProtocolError> {
        if final_client_serial < surface.committed_serial {
            return Err(ProtocolError::BadState);
        }
        surface.alive = false;
        surface.attached_buffer_provenance = 0;
        Ok(())
    }

    #[cfg(any(feature = "dw2-scene-policy", test))]
    pub fn configure_on_channel(
        &mut self,
        request: ConfigureSurface,
        received_cap_provenance: u64,
    ) -> Result<(u32, u32), ProtocolError> {
        let output_width = self.output_width;
        let output_height = self.output_height;
        let surface = self.lookup_mut_on_channel(request.identity, received_cap_provenance)?;
        Self::validate_configure(request, output_width, output_height)?;
        surface.configured_width = request.width;
        surface.configured_height = request.height;
        Ok((request.width, request.height))
    }

    #[cfg(any(feature = "dw2-scene-policy", test))]
    fn validate_configure(
        request: ConfigureSurface,
        output_width: u32,
        output_height: u32,
    ) -> Result<(), ProtocolError> {
        if request.width == 0
            || request.height == 0
            || request.width > output_width
            || request.height > output_height
            || request.configure_serial == 0
        {
            return Err(ProtocolError::BadField);
        }
        Ok(())
    }

    /// Channel peer death is surface death; idempotent with explicit destroy.
    pub fn peer_died(&mut self, cap_provenance: u64) -> bool {
        if let Some(surface) = self
            .surfaces
            .iter_mut()
            .find(|s| s.alive && s.cap_provenance == cap_provenance)
        {
            surface.alive = false;
            surface.attached_buffer_provenance = 0;
            true
        } else {
            false
        }
    }

    pub fn is_alive(&self, identity: Identity) -> bool {
        self.surfaces.iter().any(|s| {
            s.alive
                && s.id == identity.surface_id
                && s.cap_provenance == identity.surface_cap_provenance
        })
    }

    #[cfg(any(feature = "dw2-scene-policy", test))]
    pub fn alive_count(&self) -> usize {
        self.surfaces.iter().filter(|s| s.alive).count()
    }
}
