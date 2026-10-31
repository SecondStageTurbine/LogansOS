// SPDX-License-Identifier: MPL-2.0
//! Arthur OT-19 reversibility policy and artifact naming helpers.
//!
//! This module is intentionally syscall-free so it can be mounted by the host
//! Arthur logic tests. Runtime I/O lives in `main.rs`.

use alloc::string::String;

pub(crate) const UNDO_ROOT: &str = "/neofs/.arthur/undo";
pub(crate) const UNDO_ACTION_CATEGORY: &str = "actions";
pub(crate) const PREWRITE_FILE: &str = "prewrite.bin";
#[cfg_attr(not(feature = "arthur-reversibility-stakes"), allow(dead_code))]
pub(crate) const STAKES_POLICY_PATH: &str = "/neofs/.arthur/stakes.json";
pub(crate) const UNDO_STORAGE_MAX_BYTES: usize = 10 * 1024 * 1024;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

pub(crate) fn reversible_backup_allowed(path: &str) -> bool {
    !path_is_key_file(path)
}

pub(crate) fn path_is_key_file(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.ends_with(".key")
}

pub(crate) fn path_hash(path: &str) -> u64 {
    let mut hash = FNV_OFFSET;
    for byte in path.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

pub(crate) fn push_hex_u64(out: &mut String, mut value: u64) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut buf = [b'0'; 16];
    for idx in (0..buf.len()).rev() {
        buf[idx] = HEX[(value & 0x0f) as usize];
        value >>= 4;
    }
    for byte in buf {
        out.push(byte as char);
    }
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn artifact_hash_dir(path: &str) -> String {
    let mut out = String::new();
    push_hex_u64(&mut out, path_hash(path));
    out
}

pub(crate) fn prewrite_artifact_path(path: &str) -> String {
    let mut out = String::new();
    out.push_str(UNDO_ROOT);
    out.push('/');
    out.push_str(UNDO_ACTION_CATEGORY);
    out.push('/');
    push_hex_u64(&mut out, path_hash(path));
    out.push('/');
    out.push_str(PREWRITE_FILE);
    out
}

#[cfg_attr(not(feature = "arthur-reversibility-stakes"), allow(dead_code))]
pub(crate) fn stakes_policy_requires_confirm(policy_json: &str, action: &str, path: &str) -> bool {
    if !json_bool(policy_json, "confirm_destructive") {
        return false;
    }
    json_list_contains(policy_json, "paths", path) || json_list_contains(policy_json, action, path)
}

#[cfg_attr(not(feature = "arthur-reversibility-stakes"), allow(dead_code))]
fn json_bool(input: &str, key: &str) -> bool {
    let Some(mut rest) = find_json_key(input, key) else {
        return false;
    };
    rest = rest.trim_start();
    rest.starts_with("true")
}

#[cfg_attr(not(feature = "arthur-reversibility-stakes"), allow(dead_code))]
fn json_list_contains(input: &str, key: &str, needle: &str) -> bool {
    let Some(mut rest) = find_json_key(input, key) else {
        return false;
    };
    rest = rest.trim_start();
    if !rest.starts_with('[') {
        return false;
    }
    let Some(end) = rest.find(']') else {
        return false;
    };
    let list = &rest[..end];
    let mut quoted = String::new();
    quoted.push('"');
    quoted.push_str(needle);
    quoted.push('"');
    list.contains(quoted.as_str())
}

#[cfg_attr(not(feature = "arthur-reversibility-stakes"), allow(dead_code))]
fn find_json_key<'a>(input: &'a str, key: &str) -> Option<&'a str> {
    let mut quoted = String::new();
    quoted.push('"');
    quoted.push_str(key);
    quoted.push('"');
    let start = input.find(quoted.as_str())?;
    let after_key = &input[start + quoted.len()..];
    let colon = after_key.find(':')?;
    Some(&after_key[colon + 1..])
}
