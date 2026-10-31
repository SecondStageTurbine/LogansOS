// SPDX-License-Identifier: MPL-2.0
//! `cargo xtask vendor-verify`: every crate vendored by `cargo vendor` under
//! `third_party/<tree>/vendor/<crate>-<version>/` must still be byte-identical
//! to what crates.io published.
//!
//! Cargo checks a vendored crate's `.cargo-checksum.json` only when the crate
//! is reached through `[source]` replacement. The OT-61 browser closures are
//! reached through `[patch.crates-io]` path entries instead (build-std needs
//! that), so Cargo never checks them, and a stray edit to a "pristine" copy
//! would build silently (the batch-35 AAG note on #245). This recomputes the
//! SHA-256 of every file a checksum file lists and compares it.
//!
//! Patched copies live outside `vendor/` (`patched/`, with their diff beside
//! them) and have no checksum file; they are not checked here.

use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// The `"files"` map of a `.cargo-checksum.json`: relative path, hex SHA-256.
///
/// The format is flat JSON written by `cargo vendor`:
/// `{"files":{"<path>":"<hex>",...},"package":"<hex>"}`. Paths are ASCII in
/// practice; JSON escapes in them (`\\`, `\"`, `\/`, `\uXXXX`) are decoded.
pub(crate) fn parse_files(json: &str) -> Result<Vec<(String, String)>, String> {
    let start = json.find("\"files\"").ok_or("no \"files\" key")?;
    let rest = &json[start + "\"files\"".len()..];
    let open = rest.find('{').ok_or("no files object")?;
    let mut chars = rest[open + 1..].chars().peekable();
    let mut out = Vec::new();
    loop {
        skip_ws(&mut chars);
        match chars.peek() {
            Some('}') => break,
            Some(',') => {
                chars.next();
                continue;
            }
            Some('"') => {}
            other => return Err(format!("unexpected {other:?} in files object")),
        }
        let path = string(&mut chars)?;
        skip_ws(&mut chars);
        if chars.next() != Some(':') {
            return Err(format!("no ':' after {path:?}"));
        }
        skip_ws(&mut chars);
        let hash = string(&mut chars)?;
        if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!("{path}: not a SHA-256: {hash:?}"));
        }
        out.push((path, hash.to_ascii_lowercase()));
    }
    Ok(out)
}

fn skip_ws(c: &mut std::iter::Peekable<std::str::Chars<'_>>) {
    while c.peek().is_some_and(|ch| ch.is_whitespace()) {
        c.next();
    }
}

fn string(c: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Result<String, String> {
    if c.next() != Some('"') {
        return Err("expected a string".into());
    }
    let mut s = String::new();
    loop {
        match c.next().ok_or("unterminated string")? {
            '"' => return Ok(s),
            '\\' => match c.next().ok_or("bad escape")? {
                'u' => {
                    let hex: String = (0..4).filter_map(|_| c.next()).collect();
                    let v = u32::from_str_radix(&hex, 16).map_err(|_| "bad \\u escape")?;
                    s.push(char::from_u32(v).ok_or("bad \\u escape")?);
                }
                'n' => s.push('\n'),
                't' => s.push('\t'),
                e => s.push(e),
            },
            ch => s.push(ch),
        }
    }
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    let d = Sha256::digest(bytes);
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// Check one crate directory against its checksum file. Returns how many
/// files it checked, or every mismatch found.
pub(crate) fn verify_crate(dir: &Path) -> Result<usize, Vec<String>> {
    let sums = dir.join(".cargo-checksum.json");
    let json = fs::read_to_string(&sums).map_err(|e| vec![format!("{}: {e}", sums.display())])?;
    let files = parse_files(&json).map_err(|e| vec![format!("{}: {e}", sums.display())])?;
    let mut bad = Vec::new();
    for (rel, want) in &files {
        let path = dir.join(rel);
        match fs::read(&path) {
            Ok(bytes) => {
                let got = sha256_hex(&bytes);
                if &got != want {
                    bad.push(format!(
                        "{}: sha256 {got}, published {want}",
                        path.display()
                    ));
                }
            }
            Err(e) => bad.push(format!("{}: {e}", path.display())),
        }
    }
    if bad.is_empty() {
        Ok(files.len())
    } else {
        Err(bad)
    }
}

/// Every `third_party/*/vendor/*/` crate directory with a checksum file.
pub(crate) fn vendored_crates(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(trees) = fs::read_dir(root.join("third_party")) else {
        return out;
    };
    for tree in trees.flatten() {
        let Ok(crates) = fs::read_dir(tree.path().join("vendor")) else {
            continue;
        };
        for c in crates.flatten() {
            if c.path().join(".cargo-checksum.json").is_file() {
                out.push(c.path());
            }
        }
    }
    out.sort();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SHA-256("abc"), FIPS 180-2 appendix B.1.
    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn sha256_known_answer() {
        assert_eq!(sha256_hex(b"abc"), ABC);
    }

    #[test]
    fn parses_cargo_vendor_format() {
        let json = format!(
            "{{\"files\":{{\"Cargo.toml\":\"{ABC}\",\"src/a\\/b.rs\":\"{}\"}},\"package\":\"{ABC}\"}}",
            ABC.to_ascii_uppercase()
        );
        let files = parse_files(&json).unwrap();
        assert_eq!(
            files,
            vec![
                ("Cargo.toml".to_string(), ABC.to_string()),
                ("src/a/b.rs".to_string(), ABC.to_string()),
            ]
        );
    }

    #[test]
    fn rejects_a_bad_hash() {
        let json = "{\"files\":{\"x\":\"abc\"},\"package\":\"\"}";
        assert!(parse_files(json).is_err());
    }

    fn scratch(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("xtask-vendor-verify-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_pristine_crate_passes_and_an_edited_one_fails() {
        let d = scratch("edit");
        fs::write(d.join("lib.rs"), b"abc").unwrap();
        fs::write(
            d.join(".cargo-checksum.json"),
            format!("{{\"files\":{{\"lib.rs\":\"{ABC}\"}},\"package\":\"{ABC}\"}}"),
        )
        .unwrap();
        assert_eq!(verify_crate(&d), Ok(1));
        fs::write(d.join("lib.rs"), b"abd").unwrap();
        let errs = verify_crate(&d).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert!(errs[0].contains("lib.rs"));
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_missing_file_fails() {
        let d = scratch("missing");
        fs::write(
            d.join(".cargo-checksum.json"),
            format!("{{\"files\":{{\"gone.rs\":\"{ABC}\"}},\"package\":\"{ABC}\"}}"),
        )
        .unwrap();
        assert!(verify_crate(&d).is_err());
        fs::remove_dir_all(&d).unwrap();
    }
}
