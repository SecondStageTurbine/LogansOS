// SPDX-License-Identifier: MPL-2.0
//! OT-66 L1: OS sign-in's credential record and policy, with no I/O.
//!
//! `docs/THINK-2026-10-27-ot66-login-session.md` §4 is the spec:
//! - [`encode`] / [`decode`]: the 160-byte `NEOCRED1` slot (§4.3), one per
//!   file (`/neofs/.system/account.cred.a` and `.b`);
//! - [`select`]: the record is the valid slot with the highest generation,
//!   and a change writes only the other file ([`Current::write_target`]), so
//!   a cut can damage only the file being written;
//! - [`derive_verifier`]: Argon2id through vault-core's one entry point, never
//!   below vault-core's floor (§4.1, §4.2);
//! - [`recovery_code`] / [`parse_recovery`]: the recovery code (§4.4);
//! - [`delay_secs`]: the wrong-password backoff (§4.4);
//! - [`encode_meta`] / [`decode_meta`]: `account.meta`, the display name.
//!
//! neo-init's WM supplies the randomness (`sys_getrandom`), reads and writes
//! the files, and draws the screens.

#![no_std]

extern crate alloc;

use alloc::vec::Vec;
pub use vault_core::KdfParams;
use vault_core::argon2id;
pub use zeroize::Zeroizing;

pub const SLOT_LEN: usize = 160;
pub const MAGIC: [u8; 8] = *b"NEOCRED1";
pub const VERSION: u16 = 1;
/// Flags bit 0: the slot carries a recovery verifier.
pub const FLAG_RECOVERY: u16 = 1;
pub const ACCOUNT_ID_LEN: usize = 16;
pub const SALT_LEN: usize = 16;
pub const VERIFIER_LEN: usize = 32;
/// The recovery verifier is the first 24 bytes of its Argon2id tag.
pub const RECOVERY_VERIFIER_LEN: usize = 24;
/// The checksum is the first 16 bytes of BLAKE3 over bytes 0..136.
pub const CHECKSUMMED_LEN: usize = 136;
pub const CHECKSUM_LEN: usize = 16;

/// Never below vault-core's floor (64 MiB, 3 passes, 1 lane): a stored slot
/// under it is refused, not honoured (§4.2, vault-core's F5 rule).
pub const FLOOR: KdfParams = vault_core::policy::FLOOR;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot {
    pub flags: u16,
    pub params: KdfParams,
    pub account_id: [u8; ACCOUNT_ID_LEN],
    pub salt: [u8; SALT_LEN],
    pub verifier: [u8; VERIFIER_LEN],
    pub recovery_salt: [u8; SALT_LEN],
    pub recovery_verifier: [u8; RECOVERY_VERIFIER_LEN],
    pub generation: u64,
}

/// Why a slot is not a record. Every one is refused alike; the kind is for
/// logs and tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotError {
    /// Not exactly 160 bytes (a torn or missing file).
    Length,
    Magic,
    Version,
    /// The checksum does not match bytes 0..136 (a torn write).
    Checksum,
    /// Reserved bytes 152..160 are not zero.
    Reserved,
    /// A flag this version does not know, or a recovery verifier present
    /// without its flag.
    Flags,
    /// Generation 0: generations start at 1.
    Generation,
    /// Argon2 parameters under [`FLOOR`] (a downgrade).
    BelowFloor,
}

fn le16(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn le32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn checksum(prefix: &[u8]) -> [u8; CHECKSUM_LEN] {
    let mut out = [0u8; CHECKSUM_LEN];
    out.copy_from_slice(&blake3::hash(prefix).as_bytes()[..CHECKSUM_LEN]);
    out
}

/// The slot's 160 bytes (§4.3's table, little-endian).
pub fn encode(slot: &Slot) -> [u8; SLOT_LEN] {
    let mut b = [0u8; SLOT_LEN];
    b[0..8].copy_from_slice(&MAGIC);
    b[8..10].copy_from_slice(&VERSION.to_le_bytes());
    b[10..12].copy_from_slice(&slot.flags.to_le_bytes());
    b[12..16].copy_from_slice(&slot.params.memory_kib.to_le_bytes());
    b[16..20].copy_from_slice(&slot.params.iterations.to_le_bytes());
    b[20..24].copy_from_slice(&slot.params.parallelism.to_le_bytes());
    b[24..40].copy_from_slice(&slot.account_id);
    b[40..56].copy_from_slice(&slot.salt);
    b[56..88].copy_from_slice(&slot.verifier);
    b[88..104].copy_from_slice(&slot.recovery_salt);
    b[104..128].copy_from_slice(&slot.recovery_verifier);
    b[128..136].copy_from_slice(&slot.generation.to_le_bytes());
    let sum = checksum(&b[..CHECKSUMMED_LEN]);
    b[136..152].copy_from_slice(&sum);
    b
}

/// Read one file's bytes as a slot. The checks run in layout order, the
/// checksum before any field is trusted, the floor last.
pub fn decode(bytes: &[u8]) -> Result<Slot, SlotError> {
    if bytes.len() != SLOT_LEN {
        return Err(SlotError::Length);
    }
    if bytes[0..8] != MAGIC {
        return Err(SlotError::Magic);
    }
    if le16(bytes, 8) != VERSION {
        return Err(SlotError::Version);
    }
    if bytes[136..152] != checksum(&bytes[..CHECKSUMMED_LEN]) {
        return Err(SlotError::Checksum);
    }
    if bytes[152..160].iter().any(|&b| b != 0) {
        return Err(SlotError::Reserved);
    }
    let flags = le16(bytes, 10);
    let mut slot = Slot {
        flags,
        params: KdfParams {
            memory_kib: le32(bytes, 12),
            iterations: le32(bytes, 16),
            parallelism: le32(bytes, 20),
        },
        account_id: [0; ACCOUNT_ID_LEN],
        salt: [0; SALT_LEN],
        verifier: [0; VERIFIER_LEN],
        recovery_salt: [0; SALT_LEN],
        recovery_verifier: [0; RECOVERY_VERIFIER_LEN],
        generation: u64::from_le_bytes(bytes[128..136].try_into().expect("8 bytes")),
    };
    slot.account_id.copy_from_slice(&bytes[24..40]);
    slot.salt.copy_from_slice(&bytes[40..56]);
    slot.verifier.copy_from_slice(&bytes[56..88]);
    slot.recovery_salt.copy_from_slice(&bytes[88..104]);
    slot.recovery_verifier.copy_from_slice(&bytes[104..128]);
    let recovery_fields_zero = bytes[88..128].iter().all(|&b| b == 0);
    if flags & !FLAG_RECOVERY != 0 || (flags & FLAG_RECOVERY == 0 && !recovery_fields_zero) {
        return Err(SlotError::Flags);
    }
    if slot.generation == 0 {
        return Err(SlotError::Generation);
    }
    if !vault_core::policy::meets_floor(slot.params) {
        return Err(SlotError::BelowFloor);
    }
    Ok(slot)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum File {
    A,
    B,
}

impl File {
    pub const fn other(self) -> Self {
        match self {
            Self::A => Self::B,
            Self::B => Self::A,
        }
    }

    pub const fn path(self) -> &'static str {
        match self {
            Self::A => CRED_A,
            Self::B => CRED_B,
        }
    }
}

pub const SYSTEM_DIR: &str = "/neofs/.system";
pub const CRED_A: &str = "/neofs/.system/account.cred.a";
pub const CRED_B: &str = "/neofs/.system/account.cred.b";
pub const META: &str = "/neofs/.system/account.meta";

/// The record: the valid slot with the highest generation, and the file it
/// is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Current {
    pub file: File,
    pub slot: Slot,
}

impl Current {
    /// A change writes only the other file: never the one holding the
    /// record, so a cut leaves the record whole (§4.3).
    pub const fn write_target(&self) -> File {
        self.file.other()
    }
}

/// Read both files (`None`: absent or unreadable) and pick the record. On
/// equal generations (both files written at account creation) `a` wins.
pub fn select(a: Option<&[u8]>, b: Option<&[u8]>) -> Option<Current> {
    let a = a.and_then(|bytes| decode(bytes).ok());
    let b = b.and_then(|bytes| decode(bytes).ok());
    match (a, b) {
        (Some(a), Some(b)) if b.generation > a.generation => Some(Current {
            file: File::B,
            slot: b,
        }),
        (Some(a), _) => Some(Current {
            file: File::A,
            slot: a,
        }),
        (None, Some(b)) => Some(Current {
            file: File::B,
            slot: b,
        }),
        (None, None) => None,
    }
}

/// Argon2id(secret, salt) at `params` through vault-core's entry point.
/// Below the floor nothing is derived.
pub fn derive_verifier(
    secret: &[u8],
    salt: &[u8; SALT_LEN],
    params: KdfParams,
) -> Option<Zeroizing<[u8; VERIFIER_LEN]>> {
    if !vault_core::policy::meets_floor(params) {
        return None;
    }
    let mut tag = Zeroizing::new([0u8; VERIFIER_LEN]);
    argon2id(secret, salt, &[], &[], params, &mut *tag).ok()?;
    Some(tag)
}

/// Byte equality whose time does not depend on where the first difference
/// is.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Whether `password` is the slot's.
pub fn verify_password(slot: &Slot, password: &[u8]) -> bool {
    derive_verifier(password, &slot.salt, slot.params)
        .is_some_and(|tag| constant_time_eq(&tag[..], &slot.verifier))
}

/// Whether `code` (canonical symbols, [`parse_recovery`]) is the slot's
/// recovery code.
pub fn verify_recovery(slot: &Slot, code: &[u8; CODE_LEN]) -> bool {
    slot.flags & FLAG_RECOVERY != 0
        && derive_verifier(code, &slot.recovery_salt, slot.params).is_some_and(|tag| {
            constant_time_eq(&tag[..RECOVERY_VERIFIER_LEN], &slot.recovery_verifier)
        })
}

/// The randomness one slot needs: its two salts (and, at creation, the
/// account id).
pub struct SlotRandom {
    pub salt: [u8; SALT_LEN],
    pub recovery_salt: [u8; SALT_LEN],
}

/// A new slot for `password` and recovery `code` at `params`, `generation`
/// (1 at creation; the record's + 1 for a change). `None` below the floor.
pub fn new_slot(
    account_id: [u8; ACCOUNT_ID_LEN],
    password: &[u8],
    code: &[u8; CODE_LEN],
    random: &SlotRandom,
    params: KdfParams,
    generation: u64,
) -> Option<Slot> {
    let verifier = derive_verifier(password, &random.salt, params)?;
    let recovery = derive_verifier(code, &random.recovery_salt, params)?;
    let mut slot = Slot {
        flags: FLAG_RECOVERY,
        params,
        account_id,
        salt: random.salt,
        verifier: *verifier,
        recovery_salt: random.recovery_salt,
        recovery_verifier: [0; RECOVERY_VERIFIER_LEN],
        generation,
    };
    slot.recovery_verifier
        .copy_from_slice(&recovery[..RECOVERY_VERIFIER_LEN]);
    Some(slot)
}

/// The recovery code's 32 symbols: Crockford's Base32 (no I, L, O, U), so
/// a code read off paper can't confuse 1/I/L or 0/O.
pub const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
/// 20 symbols, 100 bits.
pub const CODE_LEN: usize = 20;
/// Shown as four groups of five: `XXXXX-XXXXX-XXXXX-XXXXX`.
pub const CODE_SHOWN_LEN: usize = CODE_LEN + 3;

/// A recovery code from 20 random bytes: each byte's low five bits pick a
/// symbol (256 is a multiple of 32, so every symbol is equally likely).
pub fn recovery_code(random: &[u8; CODE_LEN]) -> [u8; CODE_LEN] {
    let mut code = [0u8; CODE_LEN];
    for (symbol, byte) in code.iter_mut().zip(random) {
        *symbol = ALPHABET[usize::from(byte & 31)];
    }
    code
}

/// The code as shown: groups of five joined by `-`.
pub fn format_recovery(code: &[u8; CODE_LEN]) -> [u8; CODE_SHOWN_LEN] {
    let mut out = [b'-'; CODE_SHOWN_LEN];
    for (i, &symbol) in code.iter().enumerate() {
        out[i + i / 5] = symbol;
    }
    out
}

/// What the user typed, as the code's canonical symbols: case folded,
/// dashes and spaces skipped, and Crockford's look-alikes read as what they
/// look like (`O` as 0; `I` and `L` as 1). `None` for anything else, or not
/// exactly 20 symbols.
pub fn parse_recovery(input: &[u8]) -> Option<[u8; CODE_LEN]> {
    let mut code = [0u8; CODE_LEN];
    let mut n = 0;
    for &byte in input {
        let symbol = match byte.to_ascii_uppercase() {
            b'-' | b' ' => continue,
            b'O' => b'0',
            b'I' | b'L' => b'1',
            other if ALPHABET.contains(&other) => other,
            _ => return None,
        };
        if n == CODE_LEN {
            return None;
        }
        code[n] = symbol;
        n += 1;
    }
    (n == CODE_LEN).then_some(code)
}

/// Seconds to wait before the next try is even checked, after `failures`
/// consecutive wrong answers: none for the first three, then 2^(n−3) s,
/// capped at 60 s (§4.4).
pub const fn delay_secs(failures: u32) -> u32 {
    if failures < 3 {
        return 0;
    }
    let exp = failures - 3;
    if exp >= 6 {
        return 60;
    }
    let secs = 1u32 << exp;
    if secs > 60 { 60 } else { secs }
}

/// BLAKE3 of a file's bytes (the cut scenario's "untouched file" check).
pub fn digest(bytes: &[u8]) -> [u8; 32] {
    *blake3::hash(bytes).as_bytes()
}

/// The longest display name, in bytes (UTF-8).
pub const MAX_NAME: usize = 64;

/// `account.meta`: `name=<display name>\n` then `home=/neofs\n` (§3.1; the
/// home moves under `home/<account_id>/` with L3).
pub fn encode_meta(name: &str) -> Option<Vec<u8>> {
    if name.is_empty() || name.len() > MAX_NAME || name.bytes().any(|b| b < 0x20) {
        return None;
    }
    let mut out = Vec::with_capacity(name.len() + 18);
    out.extend_from_slice(b"name=");
    out.extend_from_slice(name.as_bytes());
    out.extend_from_slice(b"\nhome=/neofs\n");
    Some(out)
}

/// The display name `account.meta` holds, if it is well formed.
pub fn decode_meta(bytes: &[u8]) -> Option<&str> {
    let text = core::str::from_utf8(bytes).ok()?;
    let mut lines = text.split('\n');
    let name = lines.next()?.strip_prefix("name=")?;
    if name.is_empty() || name.len() > MAX_NAME || name.bytes().any(|b| b < 0x20) {
        return None;
    }
    lines.next()?.strip_prefix("home=")?;
    Some(name)
}

/// `LOGIN kdf params=m:<KiB>,t:<passes>,p:<lanes> elapsed_ms=<ms>` (§4.2), the
/// timing line the shipped parameters are chosen from.
pub fn kdf_line(params: KdfParams, elapsed_ms: u64, out: &mut Vec<u8>) {
    fn num(out: &mut Vec<u8>, mut v: u64) {
        let mut digits = [0u8; 20];
        let mut n = 0;
        loop {
            digits[n] = b'0' + (v % 10) as u8;
            n += 1;
            v /= 10;
            if v == 0 {
                break;
            }
        }
        out.extend(digits[..n].iter().rev());
    }
    out.extend_from_slice(b"LOGIN kdf params=m:");
    num(out, u64::from(params.memory_kib));
    out.extend_from_slice(b",t:");
    num(out, u64::from(params.iterations));
    out.extend_from_slice(b",p:");
    num(out, u64::from(params.parallelism));
    out.extend_from_slice(b" elapsed_ms=");
    num(out, elapsed_ms);
    out.push(b'\n');
}

/// OT-66 L1 (Dexter #186 item 2; J.c., 2026-10-30): the wrong-attempt count
/// survives a reboot. It lives in `NEOFAIL1` records, two copies beside the
/// credential's in the same WM-only `.system/` (the kernel's `system-store`),
/// written alternately like it, so a cut leaves the older copy whole.
///
/// | Offset | Len | Field |
/// |---|---|---|
/// | 0 | 8 | magic `NEOFAIL1` |
/// | 8 | 2 | version, 1 (LE) |
/// | 10 | 2 | reserved, 0 |
/// | 12 | 4 | failures (LE) |
/// | 16 | 16 | the account id it counts for |
/// | 32 | 8 | generation (LE), from 1 |
/// | 40 | 8 | the first 8 bytes of BLAKE3 over bytes 0..40 |
pub mod attempts {
    use super::{ACCOUNT_ID_LEN, File};

    pub const LEN: usize = 48;
    pub const MAGIC: &[u8; 8] = b"NEOFAIL1";
    pub const VERSION: u16 = 1;
    const CHECKSUMMED_LEN: usize = 40;
    pub const PATH_A: &str = "/neofs/.system/attempts.a";
    pub const PATH_B: &str = "/neofs/.system/attempts.b";
    /// The count read when both copies are present and neither is a valid
    /// record (damage, or tampering, not a cut: a cut tears one copy only):
    /// the longest delay, fail closed.
    pub const FAIL_CLOSED: u32 = 9;

    pub const fn path(file: File) -> &'static str {
        match file {
            File::A => PATH_A,
            File::B => PATH_B,
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Attempts {
        pub failures: u32,
        pub account_id: [u8; ACCOUNT_ID_LEN],
        pub generation: u64,
    }

    fn checksum(prefix: &[u8]) -> [u8; 8] {
        let mut out = [0u8; 8];
        out.copy_from_slice(&blake3::hash(prefix).as_bytes()[..8]);
        out
    }

    pub fn encode(a: &Attempts) -> [u8; LEN] {
        let mut b = [0u8; LEN];
        b[0..8].copy_from_slice(MAGIC);
        b[8..10].copy_from_slice(&VERSION.to_le_bytes());
        b[12..16].copy_from_slice(&a.failures.to_le_bytes());
        b[16..32].copy_from_slice(&a.account_id);
        b[32..40].copy_from_slice(&a.generation.to_le_bytes());
        let sum = checksum(&b[..CHECKSUMMED_LEN]);
        b[40..48].copy_from_slice(&sum);
        b
    }

    /// The record, or `None` for anything else: wrong length, magic or
    /// version, a checksum that doesn't match (a torn write), nonzero reserved
    /// bytes, generation 0.
    pub fn decode(bytes: &[u8]) -> Option<Attempts> {
        if bytes.len() != LEN
            || &bytes[0..8] != MAGIC
            || bytes[8..10] != VERSION.to_le_bytes()
            || bytes[40..48] != checksum(&bytes[..CHECKSUMMED_LEN])
            || bytes[10..12] != [0, 0]
        {
            return None;
        }
        let word = |r: core::ops::Range<usize>| {
            let mut out = [0u8; 8];
            out[..r.len()].copy_from_slice(&bytes[r]);
            u64::from_le_bytes(out)
        };
        let generation = word(32..40);
        if generation == 0 {
            return None;
        }
        let mut account_id = [0u8; ACCOUNT_ID_LEN];
        account_id.copy_from_slice(&bytes[16..32]);
        Some(Attempts {
            failures: word(12..16) as u32,
            account_id,
            generation,
        })
    }

    /// The count to apply, and where the next write goes: the newer valid
    /// copy counting for `account` (on equal generations `a`), written next
    /// to the other file. A copy for another account (one first boot
    /// replaced) counts as absent. Both absent: 0. Both present and neither
    /// valid for any account: [`FAIL_CLOSED`].
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct State {
        pub failures: u32,
        pub generation: u64,
        pub next: File,
    }

    pub fn select(a: Option<&[u8]>, b: Option<&[u8]>, account: &[u8; ACCOUNT_ID_LEN]) -> State {
        let (da, db) = (a.and_then(decode), b.and_then(decode));
        let mine = |d: Option<Attempts>| d.filter(|d| &d.account_id == account);
        let newest = match (mine(da), mine(db)) {
            (Some(x), Some(y)) if y.generation > x.generation => Some((File::B, y)),
            (Some(x), _) => Some((File::A, x)),
            (None, Some(y)) => Some((File::B, y)),
            (None, None) => None,
        };
        if let Some((file, rec)) = newest {
            return State {
                failures: rec.failures,
                generation: rec.generation,
                next: file.other(),
            };
        }
        // Neither counts for this account. A valid copy (another account's)
        // is a fresh start; two present, damaged copies fail closed.
        let damaged = a.is_some() && b.is_some() && da.is_none() && db.is_none();
        let generation = da
            .map_or(0, |d| d.generation)
            .max(db.map_or(0, |d| d.generation));
        State {
            failures: if damaged { FAIL_CLOSED } else { 0 },
            generation,
            next: File::A,
        }
    }

    /// The record that sets the count to `failures`, one generation on.
    pub fn next(state: &State, account: &[u8; ACCOUNT_ID_LEN], failures: u32) -> Option<Attempts> {
        Some(Attempts {
            failures,
            account_id: *account,
            generation: state.generation.checked_add(1)?,
        })
    }
}

#[cfg(test)]
mod tests;
