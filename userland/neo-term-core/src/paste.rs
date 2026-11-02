// SPDX-License-Identifier: MPL-2.0
//! Paste (THINK L1a §6.1). L1a has no paste source; this is the function the
//! later clipboard rung must use, with its KAT now.
//!
//! [`sanitize`] runs first: every C0 byte except HT, LF and CR is dropped,
//! DEL is dropped, every C1 code point (U+0080-U+009F) is dropped, invalid
//! UTF-8 is dropped, and CR LF or a lone CR becomes LF. So a paste can't
//! carry ESC (no fake end marker, no reset, no sequence at all), nor `03`
//! or `04`. [`bracket`] then wraps it in `CSI 200 ~` … `CSI 201 ~` when the
//! child set bracketed-paste mode (`CSI ? 2004 h`; ctlseqs l.2821-2828).

use alloc::vec::Vec;

pub const BEGIN: &[u8] = b"\x1b[200~";
pub const END: &[u8] = b"\x1b[201~";

/// The pasted bytes with every control removed (see the module doc).
pub fn sanitize(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len());
    let mut pending_cr = false;
    for chunk in input.utf8_chunks() {
        for c in chunk.valid().chars() {
            if pending_cr {
                pending_cr = false;
                out.push(b'\n');
                if c == '\n' {
                    continue;
                }
            }
            let u = c as u32;
            match c {
                '\r' => pending_cr = true,
                '\t' | '\n' => out.push(u as u8),
                _ if u < 0x20 || u == 0x7f || (0x80..=0x9f).contains(&u) => {}
                _ => {
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                }
            }
        }
        // `chunk.invalid()` is dropped.
    }
    if pending_cr {
        out.push(b'\n');
    }
    out
}

/// What the terminal writes for a paste of `input`.
pub fn bracket(input: &[u8], bracketed_mode: bool) -> Vec<u8> {
    let clean = sanitize(input);
    if !bracketed_mode {
        return clean;
    }
    let mut out = Vec::with_capacity(clean.len() + BEGIN.len() + END.len());
    out.extend_from_slice(BEGIN);
    out.extend_from_slice(&clean);
    out.extend_from_slice(END);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// THINK §6.1's three rows (bracketed mode on).
    #[test]
    fn the_think_rows() {
        assert_eq!(
            bracket(&[0x61, 0x1b, 0x5b, 0x32, 0x30, 0x31, 0x7e, 0x62], true),
            [
                0x1b, 0x5b, 0x32, 0x30, 0x30, 0x7e, 0x61, 0x5b, 0x32, 0x30, 0x31, 0x7e, 0x62, 0x1b,
                0x5b, 0x32, 0x30, 0x31, 0x7e
            ]
        );
        assert_eq!(
            bracket(&[0x61, 0x1b, 0x63, 0x62], true),
            [
                0x1b, 0x5b, 0x32, 0x30, 0x30, 0x7e, 0x61, 0x63, 0x62, 0x1b, 0x5b, 0x32, 0x30, 0x31,
                0x7e
            ]
        );
        assert_eq!(
            bracket(&[0x61, 0x03, 0x62], true),
            [
                0x1b, 0x5b, 0x32, 0x30, 0x30, 0x7e, 0x61, 0x62, 0x1b, 0x5b, 0x32, 0x30, 0x31, 0x7e
            ]
        );
    }

    #[test]
    fn without_the_mode_the_clean_bytes_go_bare() {
        assert_eq!(bracket(b"a\x1bcb", false), b"acb");
    }

    #[test]
    fn line_endings() {
        assert_eq!(sanitize(b"a\r\nb\rc\nd\r"), b"a\nb\nc\nd\n");
        assert_eq!(sanitize(b"\r\r\n"), b"\n\n");
        assert_eq!(sanitize(b"tab\there"), b"tab\there");
    }

    #[test]
    fn every_other_control_is_dropped() {
        let all_c0: Vec<u8> = (0u8..0x20).chain([0x7f]).collect();
        assert_eq!(sanitize(&all_c0), b"\t\n\n"); // HT, LF, and CR (lone) → LF
        // C1 as UTF-8 (U+0080..U+009F, e.g. U+009B, the 8-bit CSI).
        assert_eq!(sanitize("a\u{9b}b\u{85}c".as_bytes()), b"abc");
        // Raw 8-bit C1 bytes are invalid UTF-8: dropped too.
        assert_eq!(sanitize(&[b'a', 0x9b, b'b']), b"ab");
    }

    #[test]
    fn text_survives() {
        let s = "héllo │ 世界 🙂";
        assert_eq!(sanitize(s.as_bytes()), s.as_bytes());
    }

    #[test]
    fn no_output_ever_contains_esc_or_an_end_marker_inside() {
        let nasty: Vec<u8> = (0u8..=0xff).cycle().take(4096).collect();
        let out = bracket(&nasty, true);
        let inner = &out[BEGIN.len()..out.len() - END.len()];
        assert!(!inner.contains(&0x1b));
        assert!(!inner.iter().any(|&b| b < 0x20 && b != b'\t' && b != b'\n'));
    }
}
