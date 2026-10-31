// SPDX-License-Identifier: MPL-2.0
//! Lever 2 (Plans/NOTE-2026-10-25-arthur-lever2-design.md): `ops::swiglu_f32`, which the
//! `par-elementwise` path forks across cores by element range, gives the byte-view
//! `ops::swiglu`'s bits, whole and over every `column_partition` split the executor can
//! make. The cross-core run itself is checked in QEMU (`threaded_matmul_parity`).

use super::forward_logic::column_partition;
use super::ops::{self, OpError, TensorView};
use super::tensor_logic::{Dtype, MAX_RANK, Shape};

fn values() -> Vec<f32> {
    let mut v = vec![
        0.0,
        -0.0,
        1.0,
        -1.0,
        f32::MIN_POSITIVE,
        -f32::MIN_POSITIVE / 4.0,
        f32::from_bits(1),
        88.7,
        -88.7,
        104.0,
        -104.0,
        f32::MAX,
        f32::MIN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::NAN,
        f32::from_bits(0xffc0_0001),
    ];
    let mut x = 0x5eed_1e7e_u64;
    for _ in 0..4864 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        let scale = [0.01f32, 1.0, 12.0, 300.0][(x >> 60) as usize & 3];
        v.push((((x >> 20) % 20_001) as f32 - 10_000.0) * (scale / 10_000.0));
    }
    v
}

fn byte_path(gate: &[f32], up: &[f32]) -> Vec<u8> {
    let mut d = [0u64; MAX_RANK];
    d[0] = gate.len() as u64;
    let shape = Shape::new(1, d).unwrap();
    let (g, u): (Vec<u8>, Vec<u8>) = (
        gate.iter().flat_map(|x| x.to_le_bytes()).collect(),
        up.iter().flat_map(|x| x.to_le_bytes()).collect(),
    );
    let view = |bytes| TensorView {
        dtype: Dtype::F32,
        shape: &shape,
        bytes,
    };
    let mut out = vec![0u8; gate.len() * 4];
    ops::swiglu(&view(&g), &view(&u), &mut out).unwrap();
    out
}

fn split(gate: &[f32], up: &[f32], parts: usize, shift: usize) -> Vec<u8> {
    let n = gate.len();
    let mut out = vec![0.0f32; n];
    for t in 0..parts {
        let (start, end) = column_partition(n, parts, t);
        // `shift` moves each partition's input window but not its output window:
        // the off-by-one a wrong range in the executor would make.
        let (s_in, e_in) = ((start + shift).min(n), (end + shift).min(n));
        let len = e_in - s_in;
        ops::swiglu_f32(
            &gate[s_in..e_in],
            &up[s_in..e_in],
            &mut out[start..start + len],
        )
        .unwrap();
    }
    out.iter().flat_map(|x| x.to_le_bytes()).collect()
}

#[test]
fn swiglu_f32_matches_byte_path_whole_and_split() {
    let gate = values();
    let mut up = values();
    up.rotate_left(5);
    let truth = byte_path(&gate, &up);
    let mut whole = vec![0.0f32; gate.len()];
    ops::swiglu_f32(&gate, &up, &mut whole).unwrap();
    let whole: Vec<u8> = whole.iter().flat_map(|x| x.to_le_bytes()).collect();
    assert_eq!(whole, truth, "swiglu_f32 differs from the byte path");
    for parts in 1..=8 {
        assert_eq!(
            split(&gate, &up, parts, 0),
            truth,
            "{parts}-way split differs"
        );
    }
}

/// Falsification: a split whose input windows are one element off its output windows
/// must not match, so the compare above can see a range bug.
#[test]
fn swiglu_split_off_by_one_is_caught() {
    let gate = values();
    let up = values();
    assert_ne!(split(&gate, &up, 4, 1), byte_path(&gate, &up));
}

#[test]
fn swiglu_f32_rejects_mismatched_lengths() {
    let mut out = [0.0f32; 3];
    assert_eq!(
        ops::swiglu_f32(&[1.0; 3], &[1.0; 2], &mut out),
        Err(OpError::Shape)
    );
    assert_eq!(
        ops::swiglu_f32(&[1.0; 2], &[1.0; 2], &mut out),
        Err(OpError::Shape)
    );
}
