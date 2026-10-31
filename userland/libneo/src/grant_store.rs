// SPDX-License-Identifier: MPL-2.0
//! Arthur grant store — OT-18 Slice E1.
//!
//! Arthur (Article 29) acts only on authority it has been granted. Slice 4
//! proved the *enforcement* boundary: `execute_action` checks an in-process
//! `Grant` set, and the kernel capability check (Article 29.3, "model proposes,
//! capability disposes") is the boundary by construction. This module adds the
//! *persistence/UX* layer the ratified OT-18 design (D1/D2/D4/D7) calls for: a
//! versioned, human-readable `.arthur/grants.json` that records the user's
//! durable coarse grants, reloaded at Arthur spawn so the scopes survive across
//! sessions.
//!
//! # The authority boundary (read this before trusting `grants.json`)
//!
//! **`grants.json` is NEVER the authority.** The kernel capability Arthur holds
//! is the boundary by construction (Article 29.3). The flow is two-step:
//!   1. the shell derives a narrow kernel cap per durable grant (`derive_handle`
//!      scoped to the resource) and delegates it to Arthur (`ipc::grant`);
//!   2. the grant is recorded in `grants.json` as a UX/persistence/audit record.
//!
//! The in-process `Grant` set this module builds is the *userspace policy
//! mirror* — it decides which kernel cap `execute_action` presents to a syscall.
//! It is not the enforcement layer. A compromised Arthur that fabricates entries
//! in `grants.json` (or ignores it) cannot exceed the kernel caps it actually
//! holds: a `Grant` present in the set with no backing kernel cap still fails
//! `EPERM` at the syscall. (See the `grant_in_set_without_kernel_cap_still_eperm`
//! host test, which proves exactly this.)
//!
//! # Format (`grants.json` v1)
//!
//! ```json
//! {
//!   "version": 1,
//!   "grants": [
//!     { "scope": "FileRead", "resource": "/home/user/project",
//!       "rights": ["read"], "granted_by": "user" }
//!   ]
//! }
//! ```
//!
//! * `version` — top-level format version (`1`). A reader that sees another
//!   value refuses the store (forward-compatible: a future version switches
//!   parser) and degrades to the D7 default.
//! * `scope` — one of D2's six resource-class scopes: `FileRead`, `FileWrite`,
//!   `Process`, `Network`, `Model`, `Memory`. E5 wires stable Process/Network/
//!   Model scopes into `Grant`s where the resource is durable (spawn name,
//!   `host:port`, model path). Session handles remain session-only and are not
//!   reconstructed from the store.
//! * `resource` — the scoping dimension. For file scopes a path. **v1 matching
//!   is exact-path** (the unchanged `Grant::allows` semantics); directory-prefix
//!   scoping is a named follow-on, not E1.
//! * `rights` — an array of right tokens: `read`, `write`, `lookup`, `readdir`.
//! * `granted_by` — the OT-20 seam (action provenance across principals).
//!   Present and single-Arthur (`"user"`/`"default"`) today.
//!
//! v1 is a restricted JSON: strings carry no escape sequences (a backslash is
//! rejected), unknown keys are rejected, and a malformed store degrades to the
//! D7 default — it never crashes and never widens authority.
//!
//! # No-alloc, zero-copy
//!
//! The parser borrows `resource`/`granted_by` directly out of the input buffer
//! (`GrantEntry<'a>`), writes into a caller-provided fixed array, and uses no
//! heap — so the same logic is host-testable and runs unchanged in the
//! `#![no_std]` userland binary.

use crate::action::Grant;
use crate::{FileRights, NO_NET_DIAL_CAP};

/// The only `grants.json` format version E1 understands.
pub const GRANTS_FORMAT_VERSION: u32 = 1;

/// Upper bound on durable grants loaded from a single store. Bounds memory and
/// fails a pathologically large store closed (→ D7 default).
pub const MAX_GRANTS: usize = 64;

/// Arthur's own home directory — the D7 default FileRead scope. Arthur needs to
/// read its grant store and OT-17 memories under here.
pub const ARTHUR_HOME: &str = ".arthur/";

/// D2's six resource-class scopes. File and network scopes map to runtime
/// `Grant` values; the others round-trip through the store as taxonomy seams.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ScopeKind {
    FileRead,
    FileWrite,
    Process,
    Network,
    Model,
    Memory,
}

impl ScopeKind {
    fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "FileRead" => Self::FileRead,
            "FileWrite" => Self::FileWrite,
            "Process" => Self::Process,
            "Network" => Self::Network,
            "Model" => Self::Model,
            "Memory" => Self::Memory,
            _ => return None,
        })
    }

    /// The canonical name for this scope — the inverse of `from_name`. Used by
    /// the E2 writer to emit the `"scope"` field.
    pub fn to_name(self) -> &'static str {
        match self {
            Self::FileRead => "FileRead",
            Self::FileWrite => "FileWrite",
            Self::Process => "Process",
            Self::Network => "Network",
            Self::Model => "Model",
            Self::Memory => "Memory",
        }
    }
}

/// One durable grant as recorded in `grants.json`. `resource`/`granted_by`
/// borrow out of the source buffer (zero-copy).
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct GrantEntry<'a> {
    pub scope: ScopeKind,
    pub resource: &'a str,
    pub rights: FileRights,
    pub granted_by: &'a str,
}

impl GrantEntry<'_> {
    /// A placeholder entry for initialising fixed-capacity arrays.
    pub const EMPTY: GrantEntry<'static> = GrantEntry {
        scope: ScopeKind::FileRead,
        resource: "",
        rights: FileRights(0),
        granted_by: "",
    };
}

/// Why a `grants.json` failed to load. The consumer maps any error to the D7
/// default (fail-safe); the variants exist for diagnosis and tests.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum GrantParseError {
    /// Empty input.
    Empty,
    /// Structural JSON error, control char, escape sequence, or trailing bytes.
    Malformed,
    /// `version` absent or not `GRANTS_FORMAT_VERSION`.
    BadVersion,
    /// `scope` is not one of the six D2 names.
    UnknownScope,
    /// A `rights` token is not one of `read`/`write`/`lookup`/`readdir`.
    UnknownRight,
    /// A required field (`version`, `grants`, or an entry field) is missing.
    MissingField,
    /// More than `MAX_GRANTS` (or more than the caller's buffer) entries.
    TooManyGrants,
}

/// Kill-switch + load configuration for [`build_grant_entries`].
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum GrantConfig<'a> {
    /// Kill-switch OFF (`arthur-grants` feature off): the store is never read;
    /// Arthur holds only its spawn-inherited caps. Bit-for-bit equivalent to
    /// pre-E1 behaviour — no durable grants are reconstructed.
    Disabled,
    /// Kill-switch ON. `Some(bytes)` is the raw `.arthur/grants.json`; `None`
    /// means no store file is present (first boot).
    Enabled(Option<&'a [u8]>),
}

/// Write the D7 least-privilege default into `out`: FileRead on `.arthur/`
/// (read + directory listing) so Arthur can load its own grant store and OT-17
/// memories. Returns the number of entries written (0 or 1).
pub fn default_grant_entries<'a>(out: &mut [GrantEntry<'a>]) -> usize {
    if out.is_empty() {
        return 0;
    }
    out[0] = GrantEntry {
        scope: ScopeKind::FileRead,
        // &'static str coerces to &'a str.
        resource: ARTHUR_HOME,
        rights: FileRights::READ
            .union(FileRights::LOOKUP)
            .union(FileRights::READDIR),
        granted_by: "default",
    };
    1
}

/// Build the durable grant entries Arthur should hold at startup, applying the
/// kill-switch and failing safe.
///
/// * [`GrantConfig::Disabled`] → no entries (spawn-inherited only ≡ pre-E1).
/// * [`GrantConfig::Enabled(None)`] → the D7 default only (first boot / no store).
/// * [`GrantConfig::Enabled(Some(bytes))`] → the D7 default **plus** the parsed
///   durable grants. If the store is malformed, the D7 default only (fail-safe:
///   a corrupt store never crashes and never widens authority).
///
/// Returns the number of entries written into `out`.
pub fn build_grant_entries<'a>(config: GrantConfig<'a>, out: &mut [GrantEntry<'a>]) -> usize {
    match config {
        GrantConfig::Disabled => 0,
        GrantConfig::Enabled(None) => default_grant_entries(out),
        GrantConfig::Enabled(Some(bytes)) => {
            let mut count = default_grant_entries(out);
            if count < out.len()
                && let Ok(parsed) = parse_grants(bytes, &mut out[count..])
            {
                count += parsed;
            }
            count
        }
    }
}

/// Map a parsed entry to the `Grant` `execute_action` consumes. File scopes
/// become `Grant::File`; stable E5 Process/Network/Model resources become the
/// matching non-file grants. Session handle grants (process handles, TCP socket
/// handles, loaded model handles, tensors) are intentionally not durable.
pub fn entry_to_grant<'a>(entry: &GrantEntry<'a>) -> Option<Grant<'a>> {
    match entry.scope {
        ScopeKind::FileRead | ScopeKind::FileWrite => Some(Grant::File {
            path: entry.resource,
            rights: entry.rights,
        }),
        ScopeKind::Process => process_entry_to_grant(entry.resource),
        ScopeKind::Network => network_entry_to_grant(entry.resource),
        ScopeKind::Model => {
            if entry.resource.is_empty() {
                None
            } else {
                Some(Grant::ModelLoad {
                    path: entry.resource,
                })
            }
        }
        ScopeKind::Memory => None,
    }
}

fn process_entry_to_grant(resource: &str) -> Option<Grant<'_>> {
    if resource.is_empty() {
        return None;
    }
    if resource.contains(':') {
        return None;
    }
    Some(Grant::SpawnProcess { name: resource })
}

fn network_entry_to_grant(resource: &str) -> Option<Grant<'_>> {
    let (host, port) = resource.rsplit_once(':')?;
    if host.is_empty() || port.is_empty() {
        return None;
    }
    let port = parse_u32_decimal(port)?;
    if port > u32::from(u16::MAX) {
        return None;
    }
    Some(Grant::NetDial {
        host,
        port: port as u16,
        // A parsed policy mirror has no authority by itself. Runtime grant
        // loading replaces this sentinel with the IPC-received kernel cap.
        cap: NO_NET_DIAL_CAP,
    })
}

fn parse_u32_decimal(text: &str) -> Option<u32> {
    if text.is_empty() {
        return None;
    }
    let mut value = 0u32;
    for byte in text.bytes() {
        if !byte.is_ascii_digit() {
            return None;
        }
        value = value.checked_mul(10)?.checked_add(u32::from(byte - b'0'))?;
    }
    Some(value)
}

/// Convert parsed entries into `execute_action` `Grant`s, writing into `out`.
/// Entries whose scope is not yet wired are skipped. Returns the count written.
pub fn entries_to_grants<'a>(entries: &[GrantEntry<'a>], out: &mut [Grant<'a>]) -> usize {
    let mut n = 0;
    for entry in entries {
        if n >= out.len() {
            break;
        }
        if let Some(grant) = entry_to_grant(entry) {
            out[n] = grant;
            n += 1;
        }
    }
    n
}

/// Parse a `grants.json` byte buffer into `out`. Borrows `resource`/`granted_by`
/// from `input` (zero-copy). Returns the number of entries on success.
pub fn parse_grants<'a>(
    input: &'a [u8],
    out: &mut [GrantEntry<'a>],
) -> Result<usize, GrantParseError> {
    if input.is_empty() {
        return Err(GrantParseError::Empty);
    }
    let mut parser = Parser {
        bytes: input,
        pos: 0,
    };
    let mut version: Option<u32> = None;
    let mut grant_count: Option<usize> = None;

    parser.expect(b'{')?;
    parser.skip_ws();
    if parser.peek() == Some(b'}') {
        parser.pos += 1;
    } else {
        loop {
            let key = parser.parse_string()?;
            parser.expect(b':')?;
            match key {
                "version" => {
                    if version.is_some() {
                        return Err(GrantParseError::Malformed);
                    }
                    version = Some(parser.parse_u32()?);
                }
                "grants" => {
                    if grant_count.is_some() {
                        return Err(GrantParseError::Malformed);
                    }
                    grant_count = Some(parser.parse_grant_array(out)?);
                }
                _ => return Err(GrantParseError::Malformed),
            }
            parser.skip_ws();
            match parser.bump() {
                Some(b',') => continue,
                Some(b'}') => break,
                _ => return Err(GrantParseError::Malformed),
            }
        }
    }

    parser.skip_ws();
    if parser.pos != parser.bytes.len() {
        return Err(GrantParseError::Malformed);
    }
    match version {
        Some(GRANTS_FORMAT_VERSION) => {}
        Some(_) => return Err(GrantParseError::BadVersion),
        None => return Err(GrantParseError::BadVersion),
    }
    grant_count.ok_or(GrantParseError::MissingField)
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let byte = self.bytes.get(self.pos).copied();
        if byte.is_some() {
            self.pos += 1;
        }
        byte
    }

    fn skip_ws(&mut self) {
        while let Some(byte) = self.peek() {
            if matches!(byte, b' ' | b'\t' | b'\n' | b'\r') {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn expect(&mut self, want: u8) -> Result<(), GrantParseError> {
        self.skip_ws();
        if self.bump() == Some(want) {
            Ok(())
        } else {
            Err(GrantParseError::Malformed)
        }
    }

    /// Parse a double-quoted string and return a zero-copy slice into the input.
    /// v1 rejects escape sequences and control characters.
    fn parse_string(&mut self) -> Result<&'a str, GrantParseError> {
        self.skip_ws();
        if self.bump() != Some(b'"') {
            return Err(GrantParseError::Malformed);
        }
        let start = self.pos;
        loop {
            match self.bump() {
                Some(b'"') => {
                    let raw = &self.bytes[start..self.pos - 1];
                    return core::str::from_utf8(raw).map_err(|_| GrantParseError::Malformed);
                }
                Some(b'\\') => return Err(GrantParseError::Malformed),
                Some(byte) if byte < 0x20 => return Err(GrantParseError::Malformed),
                Some(_) => {}
                None => return Err(GrantParseError::Malformed),
            }
        }
    }

    fn parse_u32(&mut self) -> Result<u32, GrantParseError> {
        self.skip_ws();
        let mut value: u32 = 0;
        let mut saw_digit = false;
        while let Some(byte) = self.peek() {
            if byte.is_ascii_digit() {
                saw_digit = true;
                value = value
                    .checked_mul(10)
                    .and_then(|v| v.checked_add(u32::from(byte - b'0')))
                    .ok_or(GrantParseError::Malformed)?;
                self.pos += 1;
            } else {
                break;
            }
        }
        if saw_digit {
            Ok(value)
        } else {
            Err(GrantParseError::Malformed)
        }
    }

    fn parse_rights(&mut self) -> Result<FileRights, GrantParseError> {
        self.expect(b'[')?;
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(FileRights(0));
        }
        let mut bits = FileRights(0);
        loop {
            let token = self.parse_string()?;
            let right = match token {
                "read" => FileRights::READ,
                "write" => FileRights::WRITE,
                "lookup" => FileRights::LOOKUP,
                "readdir" => FileRights::READDIR,
                _ => return Err(GrantParseError::UnknownRight),
            };
            bits = bits.union(right);
            self.skip_ws();
            match self.bump() {
                Some(b',') => continue,
                Some(b']') => break,
                _ => return Err(GrantParseError::Malformed),
            }
        }
        Ok(bits)
    }

    fn parse_entry(&mut self) -> Result<GrantEntry<'a>, GrantParseError> {
        self.expect(b'{')?;
        let mut scope: Option<ScopeKind> = None;
        let mut resource: Option<&'a str> = None;
        let mut rights: Option<FileRights> = None;
        let mut granted_by: Option<&'a str> = None;

        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.pos += 1;
        } else {
            loop {
                let key = self.parse_string()?;
                self.expect(b':')?;
                match key {
                    "scope" => {
                        let name = self.parse_string()?;
                        scope =
                            Some(ScopeKind::from_name(name).ok_or(GrantParseError::UnknownScope)?);
                    }
                    "resource" => resource = Some(self.parse_string()?),
                    "rights" => rights = Some(self.parse_rights()?),
                    "granted_by" => granted_by = Some(self.parse_string()?),
                    _ => return Err(GrantParseError::Malformed),
                }
                self.skip_ws();
                match self.bump() {
                    Some(b',') => continue,
                    Some(b'}') => break,
                    _ => return Err(GrantParseError::Malformed),
                }
            }
        }

        Ok(GrantEntry {
            scope: scope.ok_or(GrantParseError::MissingField)?,
            resource: resource.ok_or(GrantParseError::MissingField)?,
            rights: rights.ok_or(GrantParseError::MissingField)?,
            granted_by: granted_by.ok_or(GrantParseError::MissingField)?,
        })
    }

    fn parse_grant_array(&mut self, out: &mut [GrantEntry<'a>]) -> Result<usize, GrantParseError> {
        self.expect(b'[')?;
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(0);
        }
        let mut count = 0;
        loop {
            if count >= out.len() || count >= MAX_GRANTS {
                return Err(GrantParseError::TooManyGrants);
            }
            out[count] = self.parse_entry()?;
            count += 1;
            self.skip_ws();
            match self.bump() {
                Some(b',') => continue,
                Some(b']') => break,
                _ => return Err(GrantParseError::Malformed),
            }
        }
        Ok(count)
    }
}

// Shared no-alloc output helpers used by the E2 writer, the E3 audit renderers,
// and the E3-revoke store rewrite.
#[cfg(any(
    feature = "arthur-grant-rerequest",
    feature = "arthur-grant-audit",
    feature = "arthur-grant-revoke"
))]
fn push_bytes(out: &mut [u8], pos: &mut usize, bytes: &[u8]) -> bool {
    if *pos + bytes.len() > out.len() {
        return false;
    }
    out[*pos..*pos + bytes.len()].copy_from_slice(bytes);
    *pos += bytes.len();
    true
}

#[cfg(any(
    feature = "arthur-grant-rerequest",
    feature = "arthur-grant-audit",
    feature = "arthur-grant-revoke"
))]
fn push_decimal(out: &mut [u8], pos: &mut usize, val: u64) -> bool {
    let mut buf = [0u8; 20];
    let mut len = 0usize;
    let mut v = val;
    if v == 0 {
        buf[0] = b'0';
        len = 1;
    } else {
        while v > 0 {
            buf[len] = b'0' + (v % 10) as u8;
            len += 1;
            v /= 10;
        }
    }
    let half = len / 2;
    let mut i = 0;
    while i < half {
        buf.swap(i, len - 1 - i);
        i += 1;
    }
    push_bytes(out, pos, &buf[..len])
}

// ═══════════════════════════════════════════════════════════════════════════
// OT-18 Slice E2 — on-miss re-request (D3/D4) + the grants.json writer
// ═══════════════════════════════════════════════════════════════════════════
//
// When `execute_action` returns `ActionError::Denied`, instead of stopping
// dead (E1a), Arthur can offer to request the missing scope. E2 ships the
// *decision logic*: infer the minimal scope from the denied capability,
// surface a scope-level request, handle the user's answer (Session / Always /
// No), remember a "No" for the session (anti-nag), and serialize an "Always"
// grant into `grants.json` (which E1b's spawn-path re-derives next session).
//
// **The live effect of "yes" (Arthur acquiring a kernel cap mid-session) is
// DEFERRED to I-15/E1b.** E2 records *intent/persistence* — it does NOT mint a
// live kernel cap. State this; do not try to grant a live cap here.
//
// No new invariant. Composes E1a's `grant_store` + `action.rs` only. No new
// `unsafe`. No-alloc / no_std-clean (fixed-capacity arrays, caller-provided
// buffers).

#[cfg(feature = "arthur-grant-rerequest")]
use crate::action::RequiredCapability;

/// Maximum resource-path length storable in a `DeniedScope` (bytes).
#[cfg(feature = "arthur-grant-rerequest")]
pub const MAX_DENIED_RESOURCE_LEN: usize = 128;

/// Maximum number of distinct scopes remembered per session.
#[cfg(feature = "arthur-grant-rerequest")]
pub const MAX_DENIED_SCOPES: usize = 32;

// ── the writer (ISC-4) ──────────────────────────────────────────────────────

/// Serialize `entries` as valid `grants.json` v1 bytes into `out`. Returns the
/// number of bytes written, or `None` if `out` is too small.
///
/// The output for a known set of entries is byte-for-byte identical to the
/// hand-laid `ORACLE` fixture in the E1a host suite (the independent byte
/// oracle — a writer and reader built together can agree even when both
/// implement the spec wrong; the hand-laid oracle catches that).
#[cfg(any(feature = "arthur-grant-rerequest", feature = "arthur-grant-revoke"))]
pub fn serialize_grants(entries: &[GrantEntry<'_>], out: &mut [u8]) -> Option<usize> {
    let mut pos = 0usize;

    if !push_bytes(out, &mut pos, b"{\n  \"version\": ") {
        return None;
    }
    if !push_decimal(out, &mut pos, GRANTS_FORMAT_VERSION as u64) {
        return None;
    }

    if entries.is_empty() {
        if !push_bytes(out, &mut pos, b",\n  \"grants\": []\n}") {
            return None;
        }
        return Some(pos);
    }

    if !push_bytes(out, &mut pos, b",\n  \"grants\": [\n") {
        return None;
    }

    let last = entries.len() - 1;
    for (i, entry) in entries.iter().enumerate() {
        if !push_bytes(out, &mut pos, b"    { \"scope\": \"") {
            return None;
        }
        if !push_bytes(out, &mut pos, entry.scope.to_name().as_bytes()) {
            return None;
        }
        if !push_bytes(out, &mut pos, b"\", \"resource\": \"") {
            return None;
        }
        if !push_bytes(out, &mut pos, entry.resource.as_bytes()) {
            return None;
        }
        if !push_bytes(out, &mut pos, b"\", \"rights\": ") {
            return None;
        }
        if !push_rights_array(out, &mut pos, entry.rights) {
            return None;
        }
        if !push_bytes(out, &mut pos, b", \"granted_by\": \"") {
            return None;
        }
        if !push_bytes(out, &mut pos, entry.granted_by.as_bytes()) {
            return None;
        }
        if !push_bytes(out, &mut pos, b"\" }") {
            return None;
        }
        if i < last && !push_bytes(out, &mut pos, b",") {
            return None;
        }
        if !push_bytes(out, &mut pos, b"\n") {
            return None;
        }
    }

    if !push_bytes(out, &mut pos, b"  ]\n}") {
        return None;
    }
    Some(pos)
}

/// Write `rights` as a JSON array of right tokens in canonical bit order
/// (read, write, lookup, readdir). `FileRights(0)` → `[]`.
#[cfg(any(feature = "arthur-grant-rerequest", feature = "arthur-grant-revoke"))]
fn push_rights_array(out: &mut [u8], pos: &mut usize, rights: FileRights) -> bool {
    const TOKENS: &[(&[u8], FileRights)] = &[
        (b"read", FileRights::READ),
        (b"write", FileRights::WRITE),
        (b"lookup", FileRights::LOOKUP),
        (b"readdir", FileRights::READDIR),
    ];
    if !push_bytes(out, pos, b"[") {
        return false;
    }
    let mut first = true;
    for (token, right) in TOKENS {
        if rights.bits() & right.bits() != right.bits() {
            continue;
        }
        if !first && !push_bytes(out, pos, b", ") {
            return false;
        }
        if !push_bytes(out, pos, b"\"") {
            return false;
        }
        if !push_bytes(out, pos, token) {
            return false;
        }
        if !push_bytes(out, pos, b"\"") {
            return false;
        }
        first = false;
    }
    push_bytes(out, pos, b"]")
}

// ── scope inference (ISC-1) ─────────────────────────────────────────────────

/// Return the parent directory of `path` — the substring up to and including
/// the last `/`. For root (`/`) the parent is `/` itself. For a path with no
/// `/`, the path is returned unchanged (degenerate — not a realistic OS path).
///
/// The trailing `/` denotes a directory scope (matching the THINK's example:
/// denied `/home/project/src/lib.rs` → asks for `/home/project/`). Note: v1
/// `Grant::allows` uses exact-path matching, so the inferred scope won't
/// immediately satisfy the original action until prefix matching lands — E2
/// records the *intent*, the live effect is deferred to I-15/E1b.
#[cfg(feature = "arthur-grant-rerequest")]
pub fn parent_dir(path: &str) -> &str {
    match path.rfind('/') {
        Some(0) => "/",
        Some(idx) => &path[..=idx],
        None => path,
    }
}

/// The scope inferred from a denied stable capability. File paths, process
/// spawn names, and model-load paths are durable resources. Process handles,
/// TCP sockets, loaded model handles, tensors, and memory are session-only.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[cfg(feature = "arthur-grant-rerequest")]
pub struct InferredScope<'a> {
    pub scope: ScopeKind,
    pub resource: &'a str,
    pub rights: FileRights,
}

/// Infer the minimal scope from a denied capability. `File { path, rights }` →
/// the parent directory of `path` + the same rights; the scope kind is
/// `FileWrite` if the `WRITE` bit is set, else `FileRead`. Stable Process and
/// Model requests infer their durable resource. Session handles are not
/// inferable from the durable store.
#[cfg(feature = "arthur-grant-rerequest")]
pub fn infer_scope<'a>(required: &RequiredCapability<'a>) -> Option<InferredScope<'a>> {
    match *required {
        RequiredCapability::File { path, rights } => {
            let scope = if rights.bits() & FileRights::WRITE.bits() != 0 {
                ScopeKind::FileWrite
            } else {
                ScopeKind::FileRead
            };
            Some(InferredScope {
                scope,
                resource: parent_dir(path),
                rights,
            })
        }
        RequiredCapability::SpawnProcess { name } => Some(InferredScope {
            scope: ScopeKind::Process,
            resource: name,
            rights: FileRights(0),
        }),
        RequiredCapability::ModelLoad { path } => Some(InferredScope {
            scope: ScopeKind::Model,
            resource: path,
            rights: FileRights(0),
        }),
        _ => None,
    }
}

// ── the re-request surface (ISC-2) + decision handling (ISC-3) ──────────────

/// A scope Arthur can present to the user for granting. The conversation layer
/// renders this as "I can't [action] — I'd need [scope]. Grant it?"
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[cfg(feature = "arthur-grant-rerequest")]
pub struct ScopeRequest<'a> {
    pub scope: ScopeKind,
    pub resource: &'a str,
    pub rights: FileRights,
}

/// The user's answer to a `ScopeRequest`.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[cfg(feature = "arthur-grant-rerequest")]
pub enum GrantDecision {
    /// Grant for this session only (not persisted; live cap deferred to I-15/E1b).
    Session,
    /// Grant durably — serialize to `grants.json`, takes effect next spawn.
    Always,
    /// Deny — remember this scope so Arthur doesn't ask again this session.
    No,
}

/// Runtime kill-switch for the re-request path. `Disabled` ⇒ on `Denied`,
/// Arthur stops with "I can't do that" — no escalation, no writer — ≡ E1a.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[cfg(feature = "arthur-grant-rerequest")]
pub enum ReRequestConfig {
    Disabled,
    Enabled,
}

/// The outcome of `handle_denied` — what Arthur should do next.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[cfg(feature = "arthur-grant-rerequest")]
pub enum ReRequestOutcome<'a> {
    /// Kill-switch off — no escalation, Arthur just stops (≡ E1a).
    Disabled,
    /// The scope was already denied this session — don't ask again (anti-nag).
    AlreadyDenied,
    /// The scope can't be inferred from this action (Network/Process/etc — deferred).
    NotInferable,
    /// Arthur should present this scope request to the user.
    Request(ScopeRequest<'a>),
}

/// Decide what to do on `ActionError::Denied`. If the kill-switch is off,
/// returns `Disabled` (no escalation ≡ E1a). If the scope was already denied
/// this session, returns `AlreadyDenied` (anti-nag). If the scope is
/// inferable, returns `Request(ScopeRequest)`. Otherwise `NotInferable`.
#[cfg(feature = "arthur-grant-rerequest")]
pub fn handle_denied<'a>(
    required: &RequiredCapability<'a>,
    denied: &mut DeniedScopes,
    config: ReRequestConfig,
) -> ReRequestOutcome<'a> {
    if config == ReRequestConfig::Disabled {
        return ReRequestOutcome::Disabled;
    }
    match infer_scope(required) {
        None => ReRequestOutcome::NotInferable,
        Some(inferred) => {
            if denied.is_denied(inferred.resource, inferred.scope) {
                ReRequestOutcome::AlreadyDenied
            } else {
                ReRequestOutcome::Request(ScopeRequest {
                    scope: inferred.scope,
                    resource: inferred.resource,
                    rights: inferred.rights,
                })
            }
        }
    }
}

// ── denial memory (ISC-3 anti-nag) ──────────────────────────────────────────

/// One remembered denial — a (resource, scope) pair the user said "no" to.
/// Fixed-capacity (no alloc): the resource is copied into a byte buffer.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[cfg(feature = "arthur-grant-rerequest")]
pub struct DeniedScope {
    resource: [u8; MAX_DENIED_RESOURCE_LEN],
    resource_len: usize,
    scope: ScopeKind,
}

#[cfg(feature = "arthur-grant-rerequest")]
impl DeniedScope {
    pub const EMPTY: Self = Self {
        resource: [0; MAX_DENIED_RESOURCE_LEN],
        resource_len: 0,
        scope: ScopeKind::FileRead,
    };
}

/// Per-session set of denied scopes. Bounded by `MAX_DENIED_SCOPES`; once
/// full, additional denials are silently dropped (bounded — never panics).
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[cfg(feature = "arthur-grant-rerequest")]
pub struct DeniedScopes {
    scopes: [DeniedScope; MAX_DENIED_SCOPES],
    count: usize,
}

#[cfg(feature = "arthur-grant-rerequest")]
impl DeniedScopes {
    pub const fn new() -> Self {
        Self {
            scopes: [DeniedScope::EMPTY; MAX_DENIED_SCOPES],
            count: 0,
        }
    }

    /// True if `(resource, scope)` was previously denied this session.
    pub fn is_denied(&self, resource: &str, scope: ScopeKind) -> bool {
        let mut i = 0;
        while i < self.count {
            let s = &self.scopes[i];
            if s.scope == scope
                && s.resource_len == resource.len()
                && &s.resource[..s.resource_len] == resource.as_bytes()
            {
                return true;
            }
            i += 1;
        }
        false
    }

    /// Record a denial. If the scope is already denied or the set is full,
    /// this is a no-op (bounded, idempotent).
    pub fn deny(&mut self, resource: &str, scope: ScopeKind) {
        if self.count >= MAX_DENIED_SCOPES || self.is_denied(resource, scope) {
            return;
        }
        let slot = &mut self.scopes[self.count];
        let len = resource.len().min(MAX_DENIED_RESOURCE_LEN);
        slot.resource[..len].copy_from_slice(&resource.as_bytes()[..len]);
        slot.resource_len = len;
        slot.scope = scope;
        self.count += 1;
    }
}

#[cfg(feature = "arthur-grant-rerequest")]
impl Default for DeniedScopes {
    fn default() -> Self {
        Self::new()
    }
}

// ── grant-decision application (ISC-3/ISC-4) ────────────────────────────────

/// The result of `apply_grant_decision`.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[cfg(feature = "arthur-grant-rerequest")]
pub enum ApplyResult {
    /// "No" — the scope was recorded in `denied_scopes` (anti-nag).
    Denied,
    /// "Session" — intent recorded. The live cap acquisition is deferred to
    /// I-15/E1b's runtime-grant path. E2 records intent only.
    SessionIntent,
    /// "Always" — the grant was serialized into the output buffer. `Some(n)`
    /// = `n` bytes written; `None` = the buffer was too small.
    Always(Option<usize>),
}

/// Apply the user's grant decision. For "Always", the new entry is appended to
/// `existing` and the combined set is serialized into `out` via
/// `serialize_grants`. For "Session" and "No", `out` is not written.
#[cfg(feature = "arthur-grant-rerequest")]
pub fn apply_grant_decision<'a>(
    decision: GrantDecision,
    scope: &ScopeRequest<'a>,
    denied: &mut DeniedScopes,
    existing: &[GrantEntry<'a>],
    out: &mut [u8],
) -> ApplyResult {
    match decision {
        GrantDecision::No => {
            denied.deny(scope.resource, scope.scope);
            ApplyResult::Denied
        }
        GrantDecision::Session => {
            // Intent recorded — the live cap acquisition is deferred to
            // I-15/E1b's runtime-grant path. E2 records intent only; it does
            // NOT mint a live kernel cap.
            ApplyResult::SessionIntent
        }
        GrantDecision::Always => {
            let entry = GrantEntry {
                scope: scope.scope,
                resource: scope.resource,
                rights: scope.rights,
                granted_by: "user",
            };
            // Combine existing + new into a fixed array (no alloc), then
            // serialize. MAX_GRANTS + 1 so an existing full store plus the
            // new entry fits.
            let mut combined: [GrantEntry<'a>; MAX_GRANTS + 1] =
                [GrantEntry::EMPTY; MAX_GRANTS + 1];
            let mut n = 0usize;
            for e in existing {
                if n >= combined.len() {
                    break;
                }
                combined[n] = *e;
                n += 1;
            }
            if n < combined.len() {
                combined[n] = entry;
                n += 1;
            }
            ApplyResult::Always(serialize_grants(&combined[..n], out))
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// OT-18 Slice E3-audit — the inspection surface (D6 inspection half)
// ═══════════════════════════════════════════════════════════════════════════
//
// D6 has two halves: **inspection** (what can Arthur do? what did it do?) and
// **revocation** (remove a grant). Revocation needs `revoke_handle` on a file
// cap. ADR-061 now supplies the file-cap graph, but E3-audit remains
// inspection-only -- **revoke is DEFERRED, not built here.** E3-audit ships
// inspection only:
//
//   * **List the grant set** — grouped by scope, showing resource/rights/
//     granted_by (the OT-20 provenance seam). Human-readable rendering into a
//     caller-provided byte buffer.
//   * **Action log** — a per-session, fixed-capacity in-memory ring of recent
//     action descriptors (ActionKind + resource + outcome + principal).
//     Appended as `execute_action` runs; listed newest-first with ring-
//     eviction when full. **In-memory/session only — no persisted format.**
//
// No new invariant. Composes E1a/E2 + `action.rs` only. No new `unsafe`.
// No-alloc / no_std-clean (fixed-capacity arrays, caller-provided buffers).

#[cfg(feature = "arthur-grant-audit")]
use crate::action::ActionKind;

// ── E3 no-alloc output helpers (gated on arthur-grant-audit) ────────────────

/// `push_bytes` for `&str` — convenience wrapper.
#[cfg(feature = "arthur-grant-audit")]
fn push_str(out: &mut [u8], pos: &mut usize, s: &str) -> bool {
    push_bytes(out, pos, s.as_bytes())
}

/// Maximum resource length storable in an action-log entry (bytes).
#[cfg(feature = "arthur-grant-audit")]
pub const MAX_LOG_RESOURCE_LEN: usize = 96;

/// Maximum number of action-log entries retained per session.
#[cfg(feature = "arthur-grant-audit")]
pub const MAX_ACTION_LOG_ENTRIES: usize = 64;

// ── ISC-1: grant-set listing ────────────────────────────────────────────────

/// One row in a structured grant listing — the programmatic form the shell
/// `arthur grants` builtin consumes. The human-readable form
/// (`format_grant_listing`) renders the same data into a byte buffer.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[cfg(feature = "arthur-grant-audit")]
pub struct GrantListingEntry<'a> {
    pub scope: ScopeKind,
    pub resource: &'a str,
    pub rights: FileRights,
    pub granted_by: &'a str,
}

/// Build a structured listing from loaded grant entries. Writes into `out`
/// and returns the count. Entries are **grouped by scope** (FileRead first,
/// then FileWrite, then the deferred taxonomy seams) so the listing reads in
/// a stable, human-predictable order regardless of how the store ordered them.
#[cfg(feature = "arthur-grant-audit")]
pub fn build_grant_listing<'a>(
    entries: &[GrantEntry<'a>],
    out: &mut [GrantListingEntry<'a>],
) -> usize {
    // Stable scope ordering: FileRead, FileWrite, Process, Network, Model, Memory.
    const SCOPE_ORDER: [ScopeKind; 6] = [
        ScopeKind::FileRead,
        ScopeKind::FileWrite,
        ScopeKind::Process,
        ScopeKind::Network,
        ScopeKind::Model,
        ScopeKind::Memory,
    ];

    let mut n = 0usize;
    for &scope in SCOPE_ORDER.iter() {
        for entry in entries {
            if n >= out.len() {
                return n;
            }
            if entry.scope == scope {
                out[n] = GrantListingEntry {
                    scope: entry.scope,
                    resource: entry.resource,
                    rights: entry.rights,
                    granted_by: entry.granted_by,
                };
                n += 1;
            }
        }
    }
    n
}

/// Render a human-readable grant listing into `out` (bytes). Returns the
/// number of bytes written, or `None` if `out` is too small. Format:
///
/// ```text
/// Arthur grant set (N):
///   FileRead:
///     .arthur/  rights=read|lookup|readdir  by=default
///     /home/project/  rights=read  by=user
///   FileWrite:
///     /home/notes/  rights=write  by=user
/// ```
///
/// This is the "what can you do?" answer. The conversation layer can present
/// it directly; the shell `arthur grants` builtin can use the structured
/// `build_grant_listing` form.
#[cfg(feature = "arthur-grant-audit")]
pub fn format_grant_listing(entries: &[GrantEntry<'_>], out: &mut [u8]) -> Option<usize> {
    let mut pos = 0usize;
    let mut listing = [GrantListingEntry {
        scope: ScopeKind::FileRead,
        resource: "",
        rights: FileRights(0),
        granted_by: "",
    }; MAX_GRANTS];
    let n = build_grant_listing(entries, &mut listing);

    // Header.
    if !push_str(out, &mut pos, "Arthur grant set (") {
        return None;
    }
    if !push_decimal(out, &mut pos, n as u64) {
        return None;
    }
    if !push_str(out, &mut pos, "):\n") {
        return None;
    }

    // Group by scope — emit a scope header once, then its entries.
    let mut current_scope: Option<ScopeKind> = None;
    for row in listing.iter().take(n) {
        if current_scope != Some(row.scope) {
            current_scope = Some(row.scope);
            if !push_str(out, &mut pos, "  ") {
                return None;
            }
            if !push_str(out, &mut pos, row.scope.to_name()) {
                return None;
            }
            if !push_str(out, &mut pos, ":\n") {
                return None;
            }
        }
        if !push_str(out, &mut pos, "    ") {
            return None;
        }
        if !push_bytes(out, &mut pos, row.resource.as_bytes()) {
            return None;
        }
        if !push_str(out, &mut pos, "  rights=") {
            return None;
        }
        if !push_rights_names(out, &mut pos, row.rights) {
            return None;
        }
        if !push_str(out, &mut pos, "  by=") {
            return None;
        }
        if !push_bytes(out, &mut pos, row.granted_by.as_bytes()) {
            return None;
        }
        if !push_str(out, &mut pos, "\n") {
            return None;
        }
    }
    Some(pos)
}

/// Write `rights` as a pipe-separated list of right names in canonical bit
/// order. `FileRights(0)` → `none`.
#[cfg(feature = "arthur-grant-audit")]
fn push_rights_names(out: &mut [u8], pos: &mut usize, rights: FileRights) -> bool {
    const NAMES: &[(&[u8], FileRights)] = &[
        (b"read", FileRights::READ),
        (b"write", FileRights::WRITE),
        (b"lookup", FileRights::LOOKUP),
        (b"readdir", FileRights::READDIR),
    ];
    let mut first = true;
    for (name, right) in NAMES {
        if rights.bits() & right.bits() != right.bits() {
            continue;
        }
        if !first && !push_bytes(out, pos, b"|") {
            return false;
        }
        if !push_bytes(out, pos, name) {
            return false;
        }
        first = false;
    }
    if first {
        push_bytes(out, pos, b"none")
    } else {
        true
    }
}

// ── ISC-2: the action log (in-memory ring) ──────────────────────────────────

/// One entry in the per-session action log. `resource` is a fixed byte buffer
/// (no alloc); `principal` is the OT-20 seam (single-Arthur = "arthur" today).
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[cfg(feature = "arthur-grant-audit")]
pub struct ActionLogEntry {
    pub kind: ActionKind,
    resource: [u8; MAX_LOG_RESOURCE_LEN],
    resource_len: usize,
    /// True if the action succeeded, false if it was Denied or errored.
    pub succeeded: bool,
    /// The OT-20 principal seam — who performed the action. Single-Arthur now
    /// ("arthur"); OT-20 extends this to multi-principal provenance.
    pub principal: [u8; 16],
    principal_len: usize,
}

#[cfg(feature = "arthur-grant-audit")]
impl ActionLogEntry {
    pub const EMPTY: Self = Self {
        kind: ActionKind::ReadFile,
        resource: [0; MAX_LOG_RESOURCE_LEN],
        resource_len: 0,
        succeeded: false,
        principal: [0; 16],
        principal_len: 0,
    };

    pub fn resource_bytes(&self) -> &[u8] {
        &self.resource[..self.resource_len]
    }

    pub fn principal_bytes(&self) -> &[u8] {
        &self.principal[..self.principal_len]
    }
}

/// A per-session, fixed-capacity in-memory ring of recent actions. Newest-first
/// listing with ring-eviction when full. **In-memory/session only — not
/// persisted to disk.** Persistence is a later slice; avoiding it keeps E3
/// off the byte-oracle hook.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[cfg(feature = "arthur-grant-audit")]
pub struct ActionLog {
    entries: [ActionLogEntry; MAX_ACTION_LOG_ENTRIES],
    /// Number of entries currently stored (0..=MAX_ACTION_LOG_ENTRIES).
    count: usize,
    /// Ring write index — the next slot to write. Wraps at capacity.
    head: usize,
    /// Total actions ever recorded (monotonic; ≥ count).
    total: u64,
}

#[cfg(feature = "arthur-grant-audit")]
impl ActionLog {
    pub const fn new() -> Self {
        Self {
            entries: [ActionLogEntry::EMPTY; MAX_ACTION_LOG_ENTRIES],
            count: 0,
            head: 0,
            total: 0,
        }
    }

    /// Total actions ever recorded (monotonic counter — never resets).
    pub fn total_recorded(&self) -> u64 {
        self.total
    }

    /// Number of entries currently retained (≤ MAX_ACTION_LOG_ENTRIES).
    pub fn retained(&self) -> usize {
        self.count
    }

    /// Record an action. `resource` is the path/host/name the action targeted;
    /// `succeeded` is true for `Ok(ActionOutcome)`, false for `Denied`/`Syscall`
    /// errors. `principal` is the OT-20 seam (e.g. `"arthur"`). When the ring
    /// is full, the oldest entry is evicted (overwritten).
    pub fn record(&mut self, kind: ActionKind, resource: &str, succeeded: bool, principal: &str) {
        let slot = &mut self.entries[self.head];
        let rlen = resource.len().min(MAX_LOG_RESOURCE_LEN);
        slot.resource[..rlen].copy_from_slice(&resource.as_bytes()[..rlen]);
        slot.resource_len = rlen;
        slot.kind = kind;
        slot.succeeded = succeeded;
        let plen = principal.len().min(16);
        slot.principal[..plen].copy_from_slice(&principal.as_bytes()[..plen]);
        slot.principal_len = plen;

        self.head = (self.head + 1) % MAX_ACTION_LOG_ENTRIES;
        if self.count < MAX_ACTION_LOG_ENTRIES {
            self.count += 1;
        }
        self.total += 1;
    }

    /// Visit entries newest-first. `f` is called for each retained entry,
    /// starting with the most recently recorded. Stops early if `f` returns
    /// false.
    pub fn for_each_newest_first<F: FnMut(&ActionLogEntry) -> bool>(&self, mut f: F) {
        if self.count == 0 {
            return;
        }
        // `head` points at the next write slot; the newest entry is at
        // (head - 1) mod capacity, and we walk backward from there.
        let mut seen = 0;
        let mut idx = if self.head == 0 {
            MAX_ACTION_LOG_ENTRIES - 1
        } else {
            self.head - 1
        };
        while seen < self.count {
            if !f(&self.entries[idx]) {
                return;
            }
            if idx == 0 {
                idx = MAX_ACTION_LOG_ENTRIES - 1;
            } else {
                idx -= 1;
            }
            seen += 1;
        }
    }

    /// Render the action log as human-readable text into `out` (bytes),
    /// newest-first. Returns the bytes written, or `None` if `out` is too
    /// small. Format:
    /// ```text
    /// Arthur action log (N retained, M total):
    ///   [ok] ReadFile /hello.txt  by=arthur
    ///   [denied] WriteFile /etc/  by=arthur
    /// ```
    pub fn format_log(&self, out: &mut [u8]) -> Option<usize> {
        let mut pos = 0usize;
        if !push_str(out, &mut pos, "Arthur action log (") {
            return None;
        }
        if !push_decimal(out, &mut pos, self.count as u64) {
            return None;
        }
        if !push_str(out, &mut pos, " retained, ") {
            return None;
        }
        if !push_decimal(out, &mut pos, self.total) {
            return None;
        }
        if !push_str(out, &mut pos, " total):\n") {
            return None;
        }

        let mut writer = LogFormatWriter {
            out,
            pos: &mut pos,
            overflowed: false,
        };
        self.for_each_newest_first(|entry| writer.write_entry(entry));
        if writer.overflowed {
            return None;
        }
        Some(pos)
    }
}

#[cfg(feature = "arthur-grant-audit")]
impl Default for ActionLog {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper for `format_log` — captures the output buffer + overflow flag.
#[cfg(feature = "arthur-grant-audit")]
struct LogFormatWriter<'a> {
    out: &'a mut [u8],
    pos: &'a mut usize,
    overflowed: bool,
}

#[cfg(feature = "arthur-grant-audit")]
impl LogFormatWriter<'_> {
    fn write_str(&mut self, s: &str) -> bool {
        push_bytes(self.out, self.pos, s.as_bytes())
    }

    fn write_entry(&mut self, entry: &ActionLogEntry) -> bool {
        if !self.write_str("  [") {
            self.overflowed = true;
            return false;
        }
        if !self.write_str(if entry.succeeded { "ok" } else { "denied" }) {
            self.overflowed = true;
            return false;
        }
        if !self.write_str("] ") {
            self.overflowed = true;
            return false;
        }
        if !push_bytes(self.out, self.pos, action_kind_name(entry.kind).as_bytes()) {
            self.overflowed = true;
            return false;
        }
        if !self.write_str(" ") {
            self.overflowed = true;
            return false;
        }
        if !push_bytes(self.out, self.pos, entry.resource_bytes()) {
            self.overflowed = true;
            return false;
        }
        if !self.write_str("  by=") {
            self.overflowed = true;
            return false;
        }
        if !push_bytes(self.out, self.pos, entry.principal_bytes()) {
            self.overflowed = true;
            return false;
        }
        if !self.write_str("\n") {
            self.overflowed = true;
            return false;
        }
        true
    }
}

/// The canonical name for an `ActionKind` (for log rendering).
#[cfg(feature = "arthur-grant-audit")]
fn action_kind_name(kind: ActionKind) -> &'static str {
    match kind {
        ActionKind::ReadFile => "ReadFile",
        ActionKind::WriteFile => "WriteFile",
        ActionKind::ListDir => "ListDir",
        ActionKind::SpawnProcess => "SpawnProcess",
        ActionKind::KillProcess => "KillProcess",
        ActionKind::InspectProcess => "InspectProcess",
        ActionKind::WaitProcess => "WaitProcess",
        ActionKind::NetDial => "NetDial",
        ActionKind::NetSend => "NetSend",
        ActionKind::NetRecv => "NetRecv",
        ActionKind::Infer => "Infer",
        ActionKind::LoadModel => "LoadModel",
        ActionKind::CloseFile => "CloseFile",
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// OT-18 Slice E3-revoke — withdraw a granted scope (D6 revocation half)
// ═══════════════════════════════════════════════════════════════════════════
//
// E3-audit shipped inspection only and left the named seam `revoke_grant`. This
// is that seam, now built. D6 revocation has two layers and this module owns
// only the **store/audit** layer (the UX/persistence record); the
// **enforcement** is the kernel `revoke_handle` on the per-scope cap, called by
// the shell — NEVER this function (Article 29.3: the kernel cap is the boundary,
// `grants.json` is the audit trail). The split, restated so it cannot drift:
//
//   * enforcement (security): the shell calls `sys_revoke`/`revoke_handle` on
//     the retained per-scope cap → that cap AND every descendant die at the
//     kernel, transitively through the derivation graph. A compromised holder
//     loses the authority immediately, regardless of `grants.json`.
//   * store (this fn): rewrite `grants.json` with the revoked entry removed so
//     the scope is not re-derived at next spawn. Best-effort, NEVER authority:
//     the cap is already dead even if this rewrite fails or the store is stale.
//
// Idempotent + minimal (ISC-4): removing an absent scope is a no-op success
// (`removed == false`), and ONLY the named `(scope, resource)` entries are
// dropped — every surviving entry re-serializes byte-for-byte through the E2
// writer (whose output is the hand-laid ORACLE fixture), so a sibling grant
// (e.g. FileRead on a *different* path) is preserved exactly.
//
// No new invariant. Composes E1a's parser + the E2 writer. No new `unsafe`.
// No-alloc / no_std-clean (fixed-capacity arrays, caller-provided buffers).

/// Why a `revoke_grant` store rewrite could not complete. Neither variant
/// affects the *enforcement* — the kernel cap is revoked by the shell before
/// (and independently of) this store update.
#[cfg(feature = "arthur-grant-revoke")]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum RevokeError {
    /// The store bytes did not parse as `grants.json` v1. The caller treats this
    /// as "nothing removable from the store" (the cap is still revoked at the
    /// kernel); on next spawn a malformed store already degrades to the D7
    /// default, so a stale entry cannot resurrect authority.
    Malformed,
    /// `out` was too small to hold the rewritten store.
    BufferTooSmall,
}

/// The outcome of a [`revoke_grant`] store rewrite.
#[cfg(feature = "arthur-grant-revoke")]
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct RevokeOutcome {
    /// True if at least one stored entry matched `(scope, resource)` and was
    /// removed. `false` is a no-op success (idempotent — already revoked /
    /// never stored, e.g. the synthesized D7 default).
    pub removed: bool,
    /// Number of `grants.json` bytes written into `out` (the rewritten store).
    pub bytes_written: usize,
    /// Number of entries remaining in the rewritten store.
    pub remaining: usize,
}

/// Rewrite a `grants.json` v1 store with every entry matching `(scope,
/// resource)` removed, serializing the survivors into `out`. The **store/audit**
/// half of D6 revocation — NOT the enforcement (see the module section header).
///
/// Idempotent + minimal: an absent scope yields `removed == false` and the
/// store re-serialized unchanged (a no-op success); only the exact
/// `(scope, resource)` match is dropped, so sibling grants survive byte-for-byte
/// through the E2 writer. The returned `bytes_written` slice of `out` is a valid
/// `grants.json` v1 document that `parse_grants` round-trips.
#[cfg(feature = "arthur-grant-revoke")]
pub fn revoke_grant<'a>(
    store_bytes: &'a [u8],
    scope: ScopeKind,
    resource: &str,
    out: &mut [u8],
) -> Result<RevokeOutcome, RevokeError> {
    let mut entries: [GrantEntry<'a>; MAX_GRANTS] = [GrantEntry::EMPTY; MAX_GRANTS];
    let parsed = parse_grants(store_bytes, &mut entries).map_err(|_| RevokeError::Malformed)?;

    let mut kept: [GrantEntry<'a>; MAX_GRANTS] = [GrantEntry::EMPTY; MAX_GRANTS];
    let mut n = 0usize;
    let mut removed = false;
    for entry in &entries[..parsed] {
        if entry.scope == scope && entry.resource == resource {
            // Minimal: drop ONLY the named scope's entry/entries.
            removed = true;
            continue;
        }
        kept[n] = *entry;
        n += 1;
    }

    let bytes_written = serialize_grants(&kept[..n], out).ok_or(RevokeError::BufferTooSmall)?;
    Ok(RevokeOutcome {
        removed,
        bytes_written,
        remaining: n,
    })
}
