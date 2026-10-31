// SPDX-License-Identifier: MPL-2.0
//! S2-F1: the shipped fonts are their publishers' canonical sfnts, not
//! reconstructions.
//!
//! Parses the sfnt container directly (no `ttf-parser`), so a file the
//! renderer happily accepts can still fail here: the whole-file checksum must
//! equal the OpenType magic, which holds only while `head.checkSumAdjustment`
//! matches the exact bytes the publisher built.

use alloc::string::String;
use alloc::vec::Vec;

use crate::fonts::{INTER_REGULAR, PLEX_MONO_REGULAR};

/// OpenType: the u32 big-endian sum of the whole file, with
/// `head.checkSumAdjustment` in place, is 0xB1B0AFBA.
const SFNT_MAGIC_SUM: u32 = 0xB1B0_AFBA;

fn be16(b: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([b[at], b[at + 1]])
}

fn be32(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn whole_file_sum(font: &[u8]) -> u32 {
    font.chunks(4).fold(0u32, |sum, chunk| {
        let mut word = [0u8; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum.wrapping_add(u32::from_be_bytes(word))
    })
}

fn table<'a>(font: &'a [u8], tag: &[u8; 4]) -> &'a [u8] {
    let count = be16(font, 4) as usize;
    (0..count)
        .map(|i| 12 + 16 * i)
        .find(|&rec| &font[rec..rec + 4] == tag)
        .map(|rec| {
            let (off, len) = (be32(font, rec + 8) as usize, be32(font, rec + 12) as usize);
            &font[off..off + len]
        })
        .expect("table present")
}

/// The Windows/Unicode/en-US record for `id` (platform 3, encoding 1,
/// language 0x409), decoded from UTF-16BE.
fn name(font: &[u8], id: u16) -> String {
    let t = table(font, b"name");
    let (count, strings) = (be16(t, 2) as usize, be16(t, 4) as usize);
    let rec = (0..count)
        .map(|i| 6 + 12 * i)
        .find(|&r| {
            be16(t, r) == 3
                && be16(t, r + 2) == 1
                && be16(t, r + 4) == 0x409
                && be16(t, r + 6) == id
        })
        .expect("name record present");
    let (len, off) = (be16(t, rec + 8) as usize, be16(t, rec + 10) as usize);
    let units: Vec<u16> = t[strings + off..strings + off + len]
        .chunks(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16(&units).expect("UTF-16BE name")
}

const OFL_NOTICE: &str = "This Font Software is licensed under the SIL Open Font License, \
     Version 1.1. This license is available with a FAQ at: http://scripts.sil.org/OFL";

#[test]
fn shipped_fonts_are_canonical_sfnts() {
    for (font, family, version, copyright) in [
        (
            INTER_REGULAR,
            "Inter",
            "Version 4.001;git-9221beed3",
            "Copyright 2016 The Inter Project Authors",
        ),
        (
            PLEX_MONO_REGULAR,
            "IBM Plex Mono",
            "Version 2.005",
            "Copyright 2017 IBM Corp. All rights reserved.",
        ),
    ] {
        assert_eq!(be32(font, 0), 0x0001_0000, "{family}: TrueType sfnt");
        assert_eq!(whole_file_sum(font), SFNT_MAGIC_SUM, "{family}: checksum");
        assert_eq!(name(font, 0), copyright);
        assert_eq!(name(font, 1), family);
        assert_eq!(name(font, 2), "Regular");
        assert_eq!(name(font, 5), version);
        assert_eq!(name(font, 13), OFL_NOTICE);
        assert_eq!(name(font, 14), "http://scripts.sil.org/OFL");
    }
}

/// The check is not vacuous: one flipped byte anywhere breaks the sum.
#[test]
fn checksum_detects_a_modified_font() {
    let mut copy = PLEX_MONO_REGULAR.to_vec();
    let at = copy.len() / 2;
    copy[at] ^= 0x01;
    assert_ne!(whole_file_sum(&copy), SFNT_MAGIC_SUM);
}
