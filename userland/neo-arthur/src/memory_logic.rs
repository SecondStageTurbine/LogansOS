// SPDX-License-Identifier: MPL-2.0
//! Arthur M1 memory record format and fail-safe helpers.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

pub const MEMORY_FORMAT_VERSION: u8 = 1;
pub const MEMORY_ROOT_PATH: &str = "/neofs/.arthur/memory/";
pub const MAX_MEMORY_CONTENT_BYTES: usize = 128;
pub const MAX_MEMORY_FILE_BYTES: usize = 2048;
pub const MAX_LOADED_RECORDS: usize = 16;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MemoryType {
    Fact,
    Preference,
    Event,
    Relationship,
}

impl MemoryType {
    #[allow(dead_code)]
    pub const ALL: [Self; 4] = [
        Self::Fact,
        Self::Preference,
        Self::Event,
        Self::Relationship,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fact => "fact",
            Self::Preference => "preference",
            Self::Event => "event",
            Self::Relationship => "relationship",
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct MemoryRecord<'a> {
    pub kind: MemoryType,
    pub timestamp: u64,
    pub content: &'a str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnedMemoryRecord {
    pub kind: MemoryType,
    pub timestamp: u64,
    pub content: String,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MemoryError {
    BadVersion,
    BadType,
    BadTimestamp,
    BadFieldCount,
    BadUtf8,
    BadContent,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum StoreFailure {
    MissingDirectory,
    MissingCategory,
    OpenDenied,
    ReadError,
    WriteError,
    NoSpace,
    CorruptRecord,
    InvalidCategory,
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MemoryMode {
    Durable,
    Ephemeral,
}

#[allow(dead_code)]
pub fn fail_safe_mode(result: Result<(), StoreFailure>) -> MemoryMode {
    match result {
        Ok(()) => MemoryMode::Durable,
        Err(_) => MemoryMode::Ephemeral,
    }
}

pub fn parse_memory_type(text: &str) -> Result<MemoryType, MemoryError> {
    match text {
        "fact" => Ok(MemoryType::Fact),
        "preference" => Ok(MemoryType::Preference),
        "event" => Ok(MemoryType::Event),
        "relationship" => Ok(MemoryType::Relationship),
        _ => Err(MemoryError::BadType),
    }
}

pub fn encode_record(record: MemoryRecord<'_>, out: &mut Vec<u8>) -> Result<(), MemoryError> {
    validate_content(record.content)?;
    out.clear();
    out.push(b'0' + MEMORY_FORMAT_VERSION);
    out.push(b':');
    out.extend_from_slice(record.kind.as_str().as_bytes());
    out.push(b':');
    push_decimal(record.timestamp, out);
    out.push(b':');
    out.extend_from_slice(record.content.as_bytes());
    out.push(b'\n');
    Ok(())
}

pub fn parse_record_line(line: &[u8]) -> Result<MemoryRecord<'_>, MemoryError> {
    let line = trim_line_ending(line);
    let text = core::str::from_utf8(line).map_err(|_| MemoryError::BadUtf8)?;
    let mut fields = text.splitn(4, ':');
    let version = fields.next().ok_or(MemoryError::BadFieldCount)?;
    let kind = fields.next().ok_or(MemoryError::BadFieldCount)?;
    let timestamp = fields.next().ok_or(MemoryError::BadFieldCount)?;
    let content = fields.next().ok_or(MemoryError::BadFieldCount)?;
    if fields.next().is_some() {
        return Err(MemoryError::BadFieldCount);
    }
    if version != "1" {
        return Err(MemoryError::BadVersion);
    }
    let kind = parse_memory_type(kind)?;
    let timestamp = parse_u64(timestamp)?;
    validate_content(content)?;
    Ok(MemoryRecord {
        kind,
        timestamp,
        content,
    })
}

pub fn parse_records_lossy(bytes: &[u8], out: &mut Vec<OwnedMemoryRecord>) -> usize {
    out.clear();
    let mut malformed = 0usize;
    for line in bytes.split(|byte| *byte == b'\n') {
        if line.is_empty() {
            continue;
        }
        match parse_record_line(line) {
            Ok(record) if out.len() < MAX_LOADED_RECORDS => out.push(OwnedMemoryRecord {
                kind: record.kind,
                timestamp: record.timestamp,
                content: String::from(record.content),
            }),
            Ok(_) => {}
            Err(_) => malformed = malformed.saturating_add(1),
        }
    }
    malformed
}

pub fn category_path(kind: MemoryType, out: &mut Vec<u8>) {
    out.clear();
    out.extend_from_slice(MEMORY_ROOT_PATH.as_bytes());
    out.extend_from_slice(kind.as_str().as_bytes());
    out.extend_from_slice(b".mem");
}

pub enum MemoryCommand<'a> {
    Remember(MemoryRecord<'a>),
    Recall(MemoryType),
    /// Crypto-erase a category (M2 only): destroy the key, reset to empty.
    #[cfg(feature = "arthur-memory-encryption")]
    Forget(MemoryType),
    /// Crypto-erase every category, ordered keys-burned-first (M4).
    #[cfg(feature = "arthur-memory-forgetting")]
    ForgetEverything,
    /// Delegate a per-category memory object cap through the existing I-15 graph.
    #[cfg(feature = "arthur-memory-delegation")]
    Delegate(MemoryType, MemoryDelegationAccess),
    /// Revoke any outstanding delegated cap for this category.
    #[cfg(feature = "arthur-memory-delegation")]
    RevokeDelegation(MemoryType),
}

#[cfg(feature = "arthur-memory-delegation")]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum MemoryDelegationAccess {
    Read,
    ReadWrite,
}

#[cfg(feature = "arthur-memory-delegation")]
impl MemoryDelegationAccess {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::ReadWrite => "write",
        }
    }
}

pub fn parse_memory_command(input: &str) -> Option<Result<MemoryCommand<'_>, MemoryError>> {
    let text = trim_prompt(input);
    if let Some(rest) = text.strip_prefix("remember ") {
        let mut fields = rest.splitn(3, ' ');
        let kind = match fields.next().ok_or(MemoryError::BadFieldCount) {
            Ok(kind) => kind,
            Err(err) => return Some(Err(err)),
        };
        let timestamp = match fields.next().ok_or(MemoryError::BadFieldCount) {
            Ok(timestamp) => timestamp,
            Err(err) => return Some(Err(err)),
        };
        let content = match fields.next().ok_or(MemoryError::BadFieldCount) {
            Ok(content) => content,
            Err(err) => return Some(Err(err)),
        };
        return Some(
            parse_memory_type(kind)
                .and_then(|kind| {
                    Ok(MemoryRecord {
                        kind,
                        timestamp: parse_u64(timestamp)?,
                        content,
                    })
                })
                .and_then(|record| {
                    validate_content(record.content)?;
                    Ok(MemoryCommand::Remember(record))
                }),
        );
    }
    if let Some(rest) = text.strip_prefix("recall ") {
        return Some(parse_memory_type(rest).map(MemoryCommand::Recall));
    }
    #[cfg(feature = "arthur-memory-forgetting")]
    if matches!(
        text,
        "forget everything"
            | "forget all memory"
            | "forget all memories"
            | "forget everything about me"
    ) {
        return Some(Ok(MemoryCommand::ForgetEverything));
    }
    #[cfg(feature = "arthur-memory-encryption")]
    if let Some(rest) = text.strip_prefix("forget ") {
        return Some(parse_memory_type(rest).map(MemoryCommand::Forget));
    }
    #[cfg(feature = "arthur-memory-delegation")]
    if let Some(rest) = text.strip_prefix("delegate ") {
        let mut fields = rest.splitn(2, ' ');
        let kind = match fields.next().ok_or(MemoryError::BadFieldCount) {
            Ok(kind) => kind,
            Err(err) => return Some(Err(err)),
        };
        let access = match fields.next().ok_or(MemoryError::BadFieldCount) {
            Ok("read") => MemoryDelegationAccess::Read,
            Ok("write") => MemoryDelegationAccess::ReadWrite,
            Ok(_) => return Some(Err(MemoryError::BadContent)),
            Err(err) => return Some(Err(err)),
        };
        return Some(parse_memory_type(kind).map(|kind| MemoryCommand::Delegate(kind, access)));
    }
    #[cfg(feature = "arthur-memory-delegation")]
    if let Some(rest) = text.strip_prefix("revoke-memory ") {
        return Some(parse_memory_type(rest).map(MemoryCommand::RevokeDelegation));
    }
    None
}

fn trim_prompt(text: &str) -> &str {
    text.trim_matches(|ch| ch == '\r' || ch == '\n')
}

fn trim_line_ending(line: &[u8]) -> &[u8] {
    match line {
        [rest @ .., b'\r'] => rest,
        _ => line,
    }
}

fn validate_content(content: &str) -> Result<(), MemoryError> {
    if content.is_empty()
        || content.len() > MAX_MEMORY_CONTENT_BYTES
        || content.bytes().any(|byte| byte == b'\n' || byte == b'\r')
    {
        return Err(MemoryError::BadContent);
    }
    Ok(())
}

fn parse_u64(text: &str) -> Result<u64, MemoryError> {
    if text.is_empty() {
        return Err(MemoryError::BadTimestamp);
    }
    let mut value = 0u64;
    for byte in text.bytes() {
        if !byte.is_ascii_digit() {
            return Err(MemoryError::BadTimestamp);
        }
        value = value
            .checked_mul(10)
            .and_then(|value| value.checked_add(u64::from(byte - b'0')))
            .ok_or(MemoryError::BadTimestamp)?;
    }
    Ok(value)
}

fn push_decimal(mut value: u64, out: &mut Vec<u8>) {
    let mut buf = [0u8; 20];
    let mut idx = buf.len();
    loop {
        idx -= 1;
        buf[idx] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    out.extend_from_slice(&buf[idx..]);
}
