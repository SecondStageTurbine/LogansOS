// SPDX-License-Identifier: MPL-2.0
//! The S3 smokes' buffer geometry and hand-laid expectation, shared by the
//! process that renders (SWGL) and the one that checks (the compositor's
//! read-only view). The same pattern as S1 (`main.rs`).

pub const W: usize = 7;
pub const H: usize = 5;
/// 8 bytes of padding past each 28-byte row.
pub const STRIDE: usize = W * 4 + 8;
/// The buffer's one page: the padding and the tail are canaries.
pub const PAGE: usize = 4096;
pub const FILL: u8 = 0xCD;

/// Row 0 (GL y = 0) first. R: red; B: the scissored blue block; G: the
/// scissored green strip with alpha 0.
pub const PATTERN: [&str; H] = ["RRRRRGG", "RBBBRGG", "RBBBRGG", "RRRRRGG", "RRRRRGG"];

/// SWGL's BGRA memory order, written out by hand.
pub fn bgra(c: u8) -> [u8; 4] {
    match c {
        b'R' => [0x00, 0x00, 0xFF, 0xFF],
        b'B' => [0xFF, 0x00, 0x00, 0xFF],
        b'G' => [0x00, 0xFF, 0x00, 0x00],
        _ => unreachable!(),
    }
}

/// The whole page as it must read after the render: the pattern at
/// `y * STRIDE`, every other byte still `FILL`.
pub fn expected_page() -> Vec<u8> {
    let mut want = vec![FILL; PAGE];
    for (y, row) in PATTERN.iter().enumerate() {
        for (x, c) in row.bytes().enumerate() {
            let at = y * STRIDE + x * 4;
            want[at..at + 4].copy_from_slice(&bgra(c));
        }
    }
    want
}

/// The first byte of `seen` that differs from [`expected_page`], printed
/// with the rows, under `tag`.
pub fn first_wrong(tag: &str, seen: &[u8]) -> Option<usize> {
    let want = expected_page();
    let bad = seen.iter().zip(&want).position(|(a, b)| a != b);
    if let Some(i) = bad {
        println!(
            "{tag} first wrong byte at {i} (row {}, byte {} of the row): got {:#04x} want {:#04x}",
            i / STRIDE,
            i % STRIDE,
            seen[i],
            want[i]
        );
        for y in 0..H {
            println!(
                "{tag} row {y}: {:02x?}",
                &seen[y * STRIDE..(y + 1) * STRIDE]
            );
        }
    }
    bad
}
