// SPDX-License-Identifier: MPL-2.0
//! Answerbacks (THINK L1a §5.4): the only output sequences that make the
//! terminal write into the child's own stdin. Three, and nothing else:
//!
//! - DSR 5n → `CSI 0 n` (ctlseqs l.1384-1385);
//! - DSR 6n → `CSI row ; col R`, 1-based (l.1386-1387);
//! - DA1 (`CSI c` or `CSI 0 c`) → `CSI ? 1 ; 2 c`, VT100 with AVO (l.769-772).
//!
//! Every other report (DECXCPR `CSI ? 6 n`, secondary DA `CSI > c`, title
//! reports, DECRQSS, OSC 52) gets no answer: those are the "output makes the
//! terminal type" paths. The parser hands the terminal each CSI it doesn't
//! handle; this filter decides.

use alloc::vec::Vec;

/// The answer to one CSI sequence, if it is one of the three.
///
/// `private` is the private-marker byte (`?`, `>`, `=`, `<`) if any,
/// `intermediates` the intermediate bytes, `params` the parameters (each
/// with any sub-parameters), `final_byte` the final byte, and `cursor` the
/// 0-based `(row, col)` when the sequence was parsed.
pub fn answer(
    private: Option<u8>,
    intermediates: &[u8],
    params: &[&[u16]],
    final_byte: u8,
    cursor: (u16, u16),
) -> Option<Vec<u8>> {
    if private.is_some() || !intermediates.is_empty() {
        return None;
    }
    // One plain parameter at most, no sub-parameters.
    let p = match params {
        [] => None,
        [[v]] => Some(*v),
        _ => return None,
    };
    match (final_byte, p) {
        (b'n', Some(5)) => Some(b"\x1b[0n".to_vec()),
        (b'n', Some(6)) => {
            let mut out = Vec::with_capacity(16);
            out.extend_from_slice(b"\x1b[");
            push_decimal(&mut out, u32::from(cursor.0) + 1);
            out.push(b';');
            push_decimal(&mut out, u32::from(cursor.1) + 1);
            out.push(b'R');
            Some(out)
        }
        (b'c', None | Some(0)) => Some(b"\x1b[?1;2c".to_vec()),
        _ => None,
    }
}

fn push_decimal(out: &mut Vec<u8>, mut v: u32) {
    let mut digits = [0u8; 10];
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

#[cfg(test)]
mod tests {
    use super::*;

    /// THINK §5.3's answerback rows: after `ab` the cursor is row 0, col 2.
    #[test]
    fn the_think_rows() {
        assert_eq!(
            answer(None, &[], &[&[6]], b'n', (0, 2)).unwrap(),
            b"\x1b[1;3R"
        );
        assert_eq!(
            answer(None, &[], &[&[5]], b'n', (0, 2)).unwrap(),
            b"\x1b[0n"
        );
        assert_eq!(answer(None, &[], &[], b'c', (0, 2)).unwrap(), b"\x1b[?1;2c");
    }

    #[test]
    fn da1_with_an_explicit_zero() {
        assert_eq!(
            answer(None, &[], &[&[0]], b'c', (0, 0)).unwrap(),
            b"\x1b[?1;2c"
        );
    }

    #[test]
    fn cursor_report_is_one_based_and_multi_digit() {
        assert_eq!(
            answer(None, &[], &[&[6]], b'n', (42, 131)).unwrap(),
            b"\x1b[43;132R"
        );
        assert_eq!(
            answer(None, &[], &[&[6]], b'n', (u16::MAX, u16::MAX)).unwrap(),
            b"\x1b[65536;65536R"
        );
    }

    /// Nothing else answers.
    #[test]
    fn every_other_report_is_refused() {
        // DECXCPR, secondary and tertiary DA, private DSR.
        assert_eq!(answer(Some(b'?'), &[], &[&[6]], b'n', (0, 0)), None);
        assert_eq!(answer(Some(b'>'), &[], &[], b'c', (0, 0)), None);
        assert_eq!(answer(Some(b'='), &[], &[], b'c', (0, 0)), None);
        // Window reports (CSI 21 t, CSI 18 t), DECRQM-like forms.
        assert_eq!(answer(None, &[], &[&[21]], b't', (0, 0)), None);
        assert_eq!(answer(None, &[], &[&[18]], b't', (0, 0)), None);
        assert_eq!(answer(None, b"$", &[&[1]], b'p', (0, 0)), None);
        // DSR with other parameters, or several.
        assert_eq!(answer(None, &[], &[&[15]], b'n', (0, 0)), None);
        assert_eq!(answer(None, &[], &[&[6], &[1]], b'n', (0, 0)), None);
        assert_eq!(answer(None, &[], &[], b'n', (0, 0)), None);
        // DA with a nonzero parameter; sub-parameters.
        assert_eq!(answer(None, &[], &[&[1]], b'c', (0, 0)), None);
        assert_eq!(answer(None, &[], &[&[6, 1]], b'n', (0, 0)), None);
    }
}
