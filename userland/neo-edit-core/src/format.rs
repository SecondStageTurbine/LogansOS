// SPDX-License-Identifier: MPL-2.0
//! What counts as text, and the file's own format (THINK §3.3-§3.4).
//!
//! A file is text when it is valid UTF-8, holds no NUL byte, and fits the
//! size cap. Its line ends are kept: LF, or CRLF when every line end in the
//! file is CRLF. Inside the editor every line end is LF; `save` puts the
//! file's convention back, so an LF or CRLF file round-trips byte for byte.
//! A mixed file is read with each CRLF as one line end and saved as LF.
//! A UTF-8 byte-order mark is kept as it was.

use alloc::string::String;
use alloc::vec::Vec;

/// The largest file the editor opens (THINK §3.3, E-D4).
pub const MAX_FILE_BYTES: usize = 4 * 1024 * 1024;

const BOM: &[u8] = b"\xEF\xBB\xBF";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotText {
    TooLarge,
    InvalidUtf8,
    Nul,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineEnding {
    Lf,
    Crlf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Format {
    pub bom: bool,
    pub ending: LineEnding,
    /// The file had both CRLF and lone LF line ends; it saves as LF.
    pub mixed: bool,
}

impl Format {
    /// A new, empty document's format.
    pub const NEW: Self = Self {
        bom: false,
        ending: LineEnding::Lf,
        mixed: false,
    };
}

/// The text test (also Files' "open with the editor" test).
pub fn check(bytes: &[u8]) -> Result<(), NotText> {
    if bytes.len() > MAX_FILE_BYTES {
        return Err(NotText::TooLarge);
    }
    let text = core::str::from_utf8(bytes).map_err(|_| NotText::InvalidUtf8)?;
    if text.as_bytes().contains(&0) {
        return Err(NotText::Nul);
    }
    Ok(())
}

/// The file's bytes as editor text (LF line ends, no BOM), and its format.
pub fn load(bytes: &[u8]) -> Result<(String, Format), NotText> {
    check(bytes)?;
    let (bom, body) = match bytes.strip_prefix(BOM) {
        Some(rest) => (true, rest),
        None => (false, bytes),
    };
    // `check` proved the whole file UTF-8, and the BOM is a whole char.
    let body = core::str::from_utf8(body).map_err(|_| NotText::InvalidUtf8)?;
    let mut crlf = 0usize;
    let mut lone_lf = 0usize;
    let b = body.as_bytes();
    for (i, &c) in b.iter().enumerate() {
        if c == b'\n' {
            if i > 0 && b[i - 1] == b'\r' {
                crlf += 1;
            } else {
                lone_lf += 1;
            }
        }
    }
    let ending = if crlf > 0 && lone_lf == 0 {
        LineEnding::Crlf
    } else {
        LineEnding::Lf
    };
    let format = Format {
        bom,
        ending,
        mixed: crlf > 0 && lone_lf > 0,
    };
    Ok((normalize(body), format))
}

/// Editor text back to the file's bytes.
pub fn save(text: &str, format: &Format) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() + 3);
    if format.bom {
        out.extend_from_slice(BOM);
    }
    match format.ending {
        LineEnding::Lf => out.extend_from_slice(text.as_bytes()),
        LineEnding::Crlf => {
            for c in text.bytes() {
                if c == b'\n' {
                    out.push(b'\r');
                }
                out.push(c);
            }
        }
    }
    out
}

/// Each CRLF becomes LF; a lone CR stays. Used on load and on paste.
pub fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n")
}
