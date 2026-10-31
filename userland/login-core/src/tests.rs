// SPDX-License-Identifier: MPL-2.0
//! The `NEOCRED1` byte oracle and the policy KATs (OT-66 L1).
//!
//! The three slots below are hand-laid, independent of [`encode`] and
//! [`decode`]: each field written from THINK §4.3's table by a separate
//! script. Their bytes come from outside this crate:
//! - **the verifiers** are the P-H-C reference `argon2` CLI's output, at commit
//!   `f57e61e19229e23c4445b85494dbf7c07de721cb` (vault-core's pinned
//!   reference), built from git with `make argon2` and run as
//!   `printf '%s' "$secret" | ./argon2 "$salt" -id -t 3 -m $log2_kib -p 1 -l 32 -r`;
//! - **the checksums** are BLAKE3 (the workspace's pinned `blake3` 1.8.5, in a
//!   standalone program) over the hand-laid bytes 0..136.
//!
//! | Slot | Password | Salt | m | Recovery code / salt | Generation |
//! |---|---|---|---|---|---|
//! | `A_GEN1` | `correct horse battery staple` | `LogansOSlogin01!` | 2^16 KiB | `7K9QM2XVD4HJ8RTWB6NE` / `LogansOSrecov01!` | 1 |
//! | `B_GEN2` | `Tr0ub4dor&3 replaced` | `LogansOSlogin02!` | 2^16 KiB | the same | 2 |
//! | `FLOOR_GEN3` | `below floor demo` | `LogansOSlogin03!` | 2^15 KiB, under the floor | the same | 3 |
//!
//! All three: 3 passes, 1 lane, flags 1 (recovery present), account id
//! `10 11 .. 1f`.

use super::*;

const P1: &[u8] = b"correct horse battery staple";
const P2: &[u8] = b"Tr0ub4dor&3 replaced";
const CODE: &[u8; CODE_LEN] = b"7K9QM2XVD4HJ8RTWB6NE";

#[rustfmt::skip]
const A_GEN1: [u8; SLOT_LEN] = [
        0x4e, 0x45, 0x4f, 0x43, 0x52, 0x45, 0x44, 0x31, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, // 0
        0x03, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, // 16
        0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x4c, 0x6f, 0x67, 0x61, 0x6e, 0x73, 0x4f, 0x53, // 32
        0x6c, 0x6f, 0x67, 0x69, 0x6e, 0x30, 0x31, 0x21, 0x07, 0x9e, 0x9e, 0x43, 0x15, 0x3c, 0x01, 0x62, // 48
        0x7b, 0x7a, 0x77, 0x7c, 0xa0, 0x4c, 0x01, 0x4d, 0xf1, 0x64, 0xbc, 0xc2, 0xfe, 0x09, 0x33, 0x41, // 64
        0xef, 0xd0, 0x0d, 0xa2, 0x7d, 0x7e, 0x42, 0xe6, 0x4c, 0x6f, 0x67, 0x61, 0x6e, 0x73, 0x4f, 0x53, // 80
        0x72, 0x65, 0x63, 0x6f, 0x76, 0x30, 0x31, 0x21, 0x09, 0xb5, 0x0b, 0xff, 0xc3, 0xaa, 0x0b, 0x0a, // 96
        0x87, 0x88, 0xdc, 0x04, 0x80, 0xcd, 0x49, 0x02, 0xf0, 0x46, 0xf1, 0xd8, 0x8c, 0x35, 0xa2, 0x2d, // 112
        0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x8f, 0xd8, 0xdf, 0x11, 0x2e, 0x18, 0x61, 0xb7, // 128
        0x0b, 0x89, 0xf0, 0xf7, 0xb5, 0x44, 0x1c, 0x39, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 144
];

#[rustfmt::skip]
const B_GEN2: [u8; SLOT_LEN] = [
        0x4e, 0x45, 0x4f, 0x43, 0x52, 0x45, 0x44, 0x31, 0x01, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, // 0
        0x03, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, // 16
        0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x4c, 0x6f, 0x67, 0x61, 0x6e, 0x73, 0x4f, 0x53, // 32
        0x6c, 0x6f, 0x67, 0x69, 0x6e, 0x30, 0x32, 0x21, 0xea, 0x37, 0x0f, 0x4b, 0x54, 0xba, 0xa8, 0x07, // 48
        0x12, 0x2f, 0xcc, 0xba, 0x08, 0xcf, 0xfd, 0x4c, 0xc5, 0xa2, 0x79, 0x08, 0xda, 0x58, 0x66, 0x58, // 64
        0x14, 0x86, 0xa1, 0xf1, 0x75, 0x45, 0x88, 0x54, 0x4c, 0x6f, 0x67, 0x61, 0x6e, 0x73, 0x4f, 0x53, // 80
        0x72, 0x65, 0x63, 0x6f, 0x76, 0x30, 0x31, 0x21, 0x09, 0xb5, 0x0b, 0xff, 0xc3, 0xaa, 0x0b, 0x0a, // 96
        0x87, 0x88, 0xdc, 0x04, 0x80, 0xcd, 0x49, 0x02, 0xf0, 0x46, 0xf1, 0xd8, 0x8c, 0x35, 0xa2, 0x2d, // 112
        0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xa4, 0x53, 0x46, 0xb8, 0x83, 0xdd, 0xb4, 0xa2, // 128
        0x60, 0x92, 0x52, 0x37, 0x36, 0xcb, 0xc5, 0x6b, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 144
];

#[rustfmt::skip]
const FLOOR_GEN3: [u8; SLOT_LEN] = [
        0x4e, 0x45, 0x4f, 0x43, 0x52, 0x45, 0x44, 0x31, 0x01, 0x00, 0x01, 0x00, 0x00, 0x80, 0x00, 0x00, // 0
        0x03, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, // 16
        0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x4c, 0x6f, 0x67, 0x61, 0x6e, 0x73, 0x4f, 0x53, // 32
        0x6c, 0x6f, 0x67, 0x69, 0x6e, 0x30, 0x33, 0x21, 0x52, 0xeb, 0xc7, 0xfc, 0x4b, 0x3c, 0x07, 0xb1, // 48
        0xcb, 0x36, 0x93, 0xab, 0x64, 0x10, 0xd4, 0x99, 0xaf, 0xef, 0xb5, 0x1b, 0xaa, 0x54, 0x30, 0xa0, // 64
        0x7e, 0xa2, 0x2f, 0x25, 0x42, 0x07, 0xfb, 0xb8, 0x4c, 0x6f, 0x67, 0x61, 0x6e, 0x73, 0x4f, 0x53, // 80
        0x72, 0x65, 0x63, 0x6f, 0x76, 0x30, 0x31, 0x21, 0x09, 0xb5, 0x0b, 0xff, 0xc3, 0xaa, 0x0b, 0x0a, // 96
        0x87, 0x88, 0xdc, 0x04, 0x80, 0xcd, 0x49, 0x02, 0xf0, 0x46, 0xf1, 0xd8, 0x8c, 0x35, 0xa2, 0x2d, // 112
        0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xcc, 0x96, 0xf2, 0xd5, 0x87, 0x46, 0x10, 0x08, // 128
        0x90, 0x78, 0xaf, 0xea, 0x10, 0x05, 0x98, 0x84, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // 144
];

fn account_id() -> [u8; ACCOUNT_ID_LEN] {
    core::array::from_fn(|i| 0x10 + i as u8)
}

fn hex<const N: usize>(text: &str) -> [u8; N] {
    core::array::from_fn(|i| u8::from_str_radix(&text[2 * i..2 * i + 2], 16).unwrap())
}

fn expected_a() -> Slot {
    let tag: [u8; 32] = hex("09b50bffc3aa0b0a8788dc0480cd4902f046f1d88c35a22d6a92ac0007400872");
    Slot {
        flags: FLAG_RECOVERY,
        params: KdfParams::INTERACTIVE_V1,
        account_id: account_id(),
        salt: *b"LogansOSlogin01!",
        verifier: hex("079e9e43153c01627b7a777ca04c014df164bcc2fe093341efd00da27d7e42e6"),
        recovery_salt: *b"LogansOSrecov01!",
        recovery_verifier: tag[..RECOVERY_VERIFIER_LEN].try_into().unwrap(),
        generation: 1,
    }
}

#[test]
fn oracle_slot_round_trips_through_the_hand_laid_bytes() {
    let a = expected_a();
    assert_eq!(decode(&A_GEN1), Ok(a));
    assert_eq!(encode(&a), A_GEN1);
    let b = decode(&B_GEN2).unwrap();
    assert_eq!(b.generation, 2);
    assert_eq!(encode(&b), B_GEN2);
}

#[test]
fn oracle_verifiers_are_the_reference_cli_s() {
    let a = expected_a();
    assert!(verify_password(&a, P1));
    assert!(!verify_password(&a, P2));
    assert!(verify_recovery(&a, CODE));
    let b = decode(&B_GEN2).unwrap();
    assert!(verify_password(&b, P2));
    assert!(!verify_password(&b, P1));
    // A slot built from the same inputs is the oracle's, byte for byte.
    let random = SlotRandom {
        salt: *b"LogansOSlogin01!",
        recovery_salt: *b"LogansOSrecov01!",
    };
    let built = new_slot(
        account_id(),
        P1,
        CODE,
        &random,
        KdfParams::INTERACTIVE_V1,
        1,
    )
    .unwrap();
    assert_eq!(encode(&built), A_GEN1);
}

#[test]
fn oracle_valid_pair_selects_the_newer_and_writes_the_older() {
    let current = select(Some(&A_GEN1), Some(&B_GEN2)).unwrap();
    assert_eq!(current.file, File::B);
    assert_eq!(current.slot.generation, 2);
    assert_eq!(current.write_target(), File::A);
    // Order does not matter; equal generations pick `a` and write `b`.
    assert_eq!(select(Some(&B_GEN2), Some(&A_GEN1)).unwrap().file, File::A);
    let tie = select(Some(&A_GEN1), Some(&A_GEN1)).unwrap();
    assert_eq!((tie.file, tie.write_target()), (File::A, File::B));
}

#[test]
fn oracle_torn_newer_file_falls_back_to_the_older() {
    // A torn write: one byte of the newer slot's verifier changed.
    let mut torn = B_GEN2;
    torn[60] ^= 0x40;
    assert_eq!(decode(&torn), Err(SlotError::Checksum));
    let current = select(Some(&A_GEN1), Some(&torn)).unwrap();
    assert_eq!((current.file, current.slot.generation), (File::A, 1));
    assert!(verify_password(&current.slot, P1));
    // Cut short, or a checksum torn instead: the same.
    assert_eq!(decode(&B_GEN2[..96]), Err(SlotError::Length));
    let mut torn_sum = B_GEN2;
    torn_sum[140] ^= 1;
    assert_eq!(
        select(Some(&A_GEN1), Some(&torn_sum)).unwrap().file,
        File::A
    );
    assert_eq!(
        select(Some(&A_GEN1), Some(&B_GEN2[..96])).unwrap().file,
        File::A
    );
    // Both torn: no record (the first-boot screen, never a default).
    let mut torn_a = A_GEN1;
    torn_a[20] ^= 1;
    assert_eq!(select(Some(&torn_a), Some(&torn)), None);
    assert_eq!(select(None, None), None);
}

#[test]
fn oracle_below_floor_slot_is_refused_even_when_newest() {
    assert_eq!(decode(&FLOOR_GEN3), Err(SlotError::BelowFloor));
    let current = select(Some(&A_GEN1), Some(&FLOOR_GEN3)).unwrap();
    assert_eq!((current.file, current.slot.generation), (File::A, 1));
    // Its password never verifies: nothing is derived below the floor.
    let mut forced = decode(&A_GEN1).unwrap();
    forced.params.memory_kib = 32 * 1024;
    forced.verifier = hex("52ebc7fc4b3c07b1cb3693ab6410d499afefb51baa5430a07ea22f254207fbb8");
    forced.salt = *b"LogansOSlogin03!";
    assert!(!verify_password(&forced, b"below floor demo"));
    assert_eq!(select(None, Some(&FLOOR_GEN3)), None);
    // One short in any dimension is below the floor too.
    for (m, t, p) in [(65535, 3, 1), (65536, 2, 1), (65536, 3, 0)] {
        let low = KdfParams {
            memory_kib: m,
            iterations: t,
            parallelism: p,
        };
        assert_eq!(derive_verifier(P1, b"LogansOSlogin01!", low), None);
    }
}

#[test]
fn decode_refuses_every_malformed_field() {
    let mut bad = A_GEN1;
    bad[0] = b'X';
    assert_eq!(decode(&bad), Err(SlotError::Magic));
    let mut bad = A_GEN1;
    bad[8] = 2;
    assert_eq!(decode(&bad), Err(SlotError::Version));
    let mut bad = A_GEN1;
    bad[159] = 1;
    assert_eq!(decode(&bad), Err(SlotError::Reserved));
    // Fields past the checksum check need a re-summed slot.
    let resum = |mut slot: [u8; SLOT_LEN]| {
        let sum = blake3::hash(&slot[..CHECKSUMMED_LEN]);
        slot[136..152].copy_from_slice(&sum.as_bytes()[..CHECKSUM_LEN]);
        slot
    };
    let mut bad = A_GEN1;
    bad[10] = 3;
    assert_eq!(decode(&resum(bad)), Err(SlotError::Flags));
    let mut bad = A_GEN1;
    bad[10] = 0;
    assert_eq!(decode(&resum(bad)), Err(SlotError::Flags));
    let mut bad = A_GEN1;
    bad[128] = 0;
    assert_eq!(decode(&resum(bad)), Err(SlotError::Generation));
    assert_eq!(decode(&resum(A_GEN1)), Ok(expected_a()));
}

#[test]
fn backoff_schedule_kat() {
    // §4.4: none for three failures, then 2^(n-3) s, capped at 60 s.
    let table = [0, 0, 0, 1, 2, 4, 8, 16, 32, 60, 60, 60];
    for (failures, want) in table.iter().enumerate() {
        assert_eq!(
            delay_secs(failures as u32),
            *want,
            "after {failures} failures"
        );
    }
    assert_eq!(delay_secs(u32::MAX), 60);
}

#[test]
fn recovery_code_alphabet_and_parse() {
    assert_eq!(ALPHABET.len(), 32);
    for forbidden in *b"ILOU" {
        assert!(!ALPHABET.contains(&forbidden));
    }
    // Every byte value maps to a symbol, each symbol from exactly 8 values.
    let mut counts = [0u32; 32];
    for byte in 0..=255u8 {
        let code = recovery_code(&[byte; CODE_LEN]);
        counts[ALPHABET.iter().position(|&s| s == code[0]).unwrap()] += 1;
    }
    assert!(counts.iter().all(|&n| n == 8));
    // Bytes (37 i) mod 256: each picks ALPHABET[byte & 31] (worked by hand).
    let code = recovery_code(&core::array::from_fn(|i| (i as u8).wrapping_mul(37)));
    assert_eq!(&code, b"05AFMSY38DJQW16BGNTZ");
    assert_eq!(&format_recovery(CODE), b"7K9QM-2XVD4-HJ8RT-WB6NE");
    assert_eq!(parse_recovery(b"7K9QM-2XVD4-HJ8RT-WB6NE"), Some(*CODE));
    assert_eq!(parse_recovery(b"7k9qm 2xvd4 hj8rt wb6ne"), Some(*CODE));
    // Look-alikes read as what they look like.
    assert_eq!(
        parse_recovery(b"OIL00-00000-00000-00000"),
        Some(*b"01100000000000000000")
    );
    assert_eq!(parse_recovery(b"7K9QM-2XVD4-HJ8RT-WB6N"), None);
    assert_eq!(parse_recovery(b"7K9QM-2XVD4-HJ8RT-WB6NEE"), None);
    assert_eq!(parse_recovery(b"7K9QM-2XVD4-HJ8RT-WB6NU"), None);
    // A wrong code does not verify.
    assert!(!verify_recovery(&expected_a(), b"7K9QM2XVD4HJ8RTWB6NF"));
}

#[test]
fn meta_round_trip_and_limits() {
    let meta = encode_meta("J.c.").unwrap();
    assert_eq!(&meta, b"name=J.c.\nhome=/neofs\n");
    assert_eq!(decode_meta(&meta), Some("J.c."));
    assert_eq!(encode_meta(""), None);
    assert_eq!(encode_meta(&"x".repeat(MAX_NAME + 1)), None);
    assert!(encode_meta(&"x".repeat(MAX_NAME)).is_some());
    assert_eq!(decode_meta(b"name=a\n"), None);
    assert_eq!(decode_meta(b"nom=a\nhome=/neofs\n"), None);
}

#[test]
fn kdf_line_format() {
    let mut line = Vec::new();
    kdf_line(KdfParams::INTERACTIVE_V1, 812, &mut line);
    assert_eq!(&line, b"LOGIN kdf params=m:65536,t:3,p:1 elapsed_ms=812\n");
}

/// `NEOFAIL1` (Dexter #186 item 2): failures 4, account id `10 11 .. 1f`,
/// generation 2, laid out by hand from the [`attempts`] table. The checksum
/// is BLAKE3 (`blake3` 1.8.5, a standalone program) over these bytes 0..40.
#[rustfmt::skip]
const ATTEMPTS_GEN2: [u8; attempts::LEN] = [
    b'N', b'E', b'O', b'F', b'A', b'I', b'L', b'1',
    0x01, 0x00, 0x00, 0x00,
    0x04, 0x00, 0x00, 0x00,
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17,
    0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f,
    0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0xc6, 0xbe, 0xb8, 0x69, 0x28, 0x3e, 0xe5, 0x73,
];

const ACCOUNT: [u8; ACCOUNT_ID_LEN] = [
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f,
];

#[test]
fn oracle_attempts_round_trip_through_the_hand_laid_bytes() {
    let rec = attempts::Attempts {
        failures: 4,
        account_id: ACCOUNT,
        generation: 2,
    };
    assert_eq!(attempts::encode(&rec), ATTEMPTS_GEN2);
    assert_eq!(attempts::decode(&ATTEMPTS_GEN2), Some(rec));
}

#[test]
fn attempts_refuse_every_damaged_form() {
    let mut torn = ATTEMPTS_GEN2;
    torn[13] ^= 1;
    assert_eq!(attempts::decode(&torn), None, "checksum");
    assert_eq!(attempts::decode(&ATTEMPTS_GEN2[..47]), None, "length");
    let mut magic = ATTEMPTS_GEN2;
    magic[0] = b'X';
    assert_eq!(attempts::decode(&magic), None, "magic");
    // Reserved bytes and generation 0 checked after a valid checksum.
    let mut rec = attempts::Attempts {
        failures: 1,
        account_id: ACCOUNT,
        generation: 0,
    };
    assert_eq!(
        attempts::decode(&attempts::encode(&rec)),
        None,
        "generation 0"
    );
    rec.generation = 1;
    assert!(attempts::decode(&attempts::encode(&rec)).is_some());
}

#[test]
fn attempts_select_the_newer_copy_and_write_the_other() {
    let older = attempts::encode(&attempts::Attempts {
        failures: 3,
        account_id: ACCOUNT,
        generation: 1,
    });
    let s = attempts::select(Some(&older), Some(&ATTEMPTS_GEN2), &ACCOUNT);
    assert_eq!((s.failures, s.generation, s.next), (4, 2, File::A));
    // A cut tearing the newer copy leaves the older count, never zero.
    let mut torn = ATTEMPTS_GEN2;
    torn[20] ^= 0xff;
    let s = attempts::select(Some(&older), Some(&torn), &ACCOUNT);
    assert_eq!((s.failures, s.generation, s.next), (3, 1, File::B));
}

#[test]
fn attempts_absent_start_at_zero_and_damage_fails_closed() {
    let s = attempts::select(None, None, &ACCOUNT);
    assert_eq!((s.failures, s.generation), (0, 0));
    let mut a = ATTEMPTS_GEN2;
    a[13] ^= 1;
    let mut b = ATTEMPTS_GEN2;
    b[30] ^= 1;
    let s = attempts::select(Some(&a), Some(&b), &ACCOUNT);
    assert_eq!(s.failures, attempts::FAIL_CLOSED);
    assert!(delay_secs(attempts::FAIL_CLOSED) == 60);
    // Another account's count (first boot replaced the account) is fresh,
    // and the next write moves past its generation.
    let other = [0u8; ACCOUNT_ID_LEN];
    let s = attempts::select(Some(&ATTEMPTS_GEN2), None, &other);
    assert_eq!((s.failures, s.generation), (0, 2));
    assert_eq!(attempts::next(&s, &other, 1).map(|r| r.generation), Some(3));
}
