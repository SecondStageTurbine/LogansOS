// SPDX-License-Identifier: MPL-2.0
//! OT-86 Fix 3 (L4) known-answer tests: hand-laid AML byte programs run on the
//! stock `acpi` 6.1.1 and the patched copy through the grade's recording
//! handler, as in `fix1_kats.rs` and `fix2_kats.rs`. Opcode values come from
//! the raw ACPICA header `source/include/amlcode.h` (acpica/acpica @
//! e85aa3be); the ACPICA behaviour each test follows is named in its comment
//! and was read from that checkout.

use acpi_aml_grade::corpus::{Table, load_set};
use acpi_aml_grade::grade_acpi6::{fix1, stock};
use acpi_aml_grade::report::Kat;
use std::path::PathBuf;

fn facp() -> Table {
    let dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/acpi/qemu-q35-seabios-smp1");
    let set = load_set(&dir).unwrap();
    let t = set.find("FACP").unwrap();
    Table {
        file: t.file.clone(),
        signature: t.signature.clone(),
        bytes: t.bytes.clone(),
    }
}

/// Run `aml` (a table body, revision 2: 64-bit integers) on both crates.
fn both(aml: &[u8], calls: &[&str]) -> (Kat, Kat) {
    let f = facp();
    (
        stock::run_kat(&f, 2, aml, calls),
        fix1::run_kat(&f, 2, aml, calls),
    )
}

const TRUE: &str = "Integer(18446744073709551615)";
const FALSE: &str = "Integer(0)";

fn ok(v: &str) -> Result<String, String> {
    Ok(v.to_string())
}

// ---- Item 1: String and Buffer comparison ------------------------------------

/// ```text
/// Method (UEQ1, 0) { Return (LEqual (ToUUID ("3cdff6f7-…"), ToUUID ("3cdff6f7-…"))) }
/// Method (UNE1, 0) { Return (LEqual (ToUUID ("3cdff6f7-…"), <the same UUID, last byte 0xDF>)) }
/// ```
/// The 16-byte compare every HID-over-I2C `_DSM` makes on Arg0. ACPICA `AcpiExDoLogicalOp`
/// (`exmisc.c`): `LEqual` of two Buffers is true when the lengths and every byte match. Stock
/// copies each operand into 4 bytes and panics on the longer slice.
#[test]
fn uuid_buffers_compare_byte_by_byte() {
    #[rustfmt::skip]
    let aml = [
        0x14, 0x30, b'U', b'E', b'Q', b'1', 0x00,     // MethodOp PkgLength(48) "UEQ1" flags 0
        0xa4, 0x93,                                   //   ReturnOp LEqualOp
        0x11, 0x13, 0x0a, 0x10,                       //     BufferOp PkgLength(19) BytePrefix 16
        0xf7, 0xf6, 0xdf, 0x3c, 0x67, 0x42, 0x55, 0x45, 0xad, 0x05, 0xb3, 0x0a, 0x3d, 0x89, 0x38, 0xde,
        0x11, 0x13, 0x0a, 0x10,                       //     BufferOp PkgLength(19) BytePrefix 16
        0xf7, 0xf6, 0xdf, 0x3c, 0x67, 0x42, 0x55, 0x45, 0xad, 0x05, 0xb3, 0x0a, 0x3d, 0x89, 0x38, 0xde,
        0x14, 0x30, b'U', b'N', b'E', b'1', 0x00,     // MethodOp PkgLength(48) "UNE1" flags 0
        0xa4, 0x93,                                   //   ReturnOp LEqualOp
        0x11, 0x13, 0x0a, 0x10,                       //     BufferOp PkgLength(19) BytePrefix 16
        0xf7, 0xf6, 0xdf, 0x3c, 0x67, 0x42, 0x55, 0x45, 0xad, 0x05, 0xb3, 0x0a, 0x3d, 0x89, 0x38, 0xde,
        0x11, 0x13, 0x0a, 0x10,                       //     BufferOp PkgLength(19) BytePrefix 16
        0xf7, 0xf6, 0xdf, 0x3c, 0x67, 0x42, 0x55, 0x45, 0xad, 0x05, 0xb3, 0x0a, 0x3d, 0x89, 0x38, 0xdf,
    ];
    let (s, f) = both(&aml, &["\\UEQ1", "\\UNE1"]);
    assert_eq!(f.results, vec![ok(TRUE), ok(FALSE)]);
    for r in &s.results {
        assert!(matches!(r, Err(e) if e.starts_with("panic: ")), "{s:?}");
    }
}

/// ```text
/// Method (PFXL, 0) { Return (LLess    (Buffer (2) { 1, 2 }, Buffer (3) { 1, 2, 3 })) }
/// Method (PFXG, 0) { Return (LGreater (Buffer (2) { 1, 2 }, Buffer (3) { 1, 2, 3 })) }
/// Method (BYTG, 0) { Return (LGreater (Buffer (2) { 1, 3 }, Buffer (3) { 1, 2, 9 })) }
/// ```
/// ACPICA compares the bytes up to the shorter length, then the lengths: a prefix is less, and a
/// larger byte wins over a longer buffer. Stock compares the first 4 bytes as a little-endian
/// Integer, which agrees on the prefix pair (0x0201 < 0x030201) but not on `BYTG`
/// (0x0301 < 0x090201, so stock says false).
#[test]
fn buffers_order_lexicographically() {
    #[rustfmt::skip]
    let aml = [
        0x14, 0x15, b'P', b'F', b'X', b'L', 0x00,     // MethodOp PkgLength(21) "PFXL" flags 0
        0xa4, 0x95,                                   //   ReturnOp LLessOp
        0x11, 0x05, 0x0a, 0x02, 0x01, 0x02,           //     BufferOp PkgLength(5) 2 { 1, 2 }
        0x11, 0x06, 0x0a, 0x03, 0x01, 0x02, 0x03,     //     BufferOp PkgLength(6) 3 { 1, 2, 3 }
        0x14, 0x15, b'P', b'F', b'X', b'G', 0x00,     // MethodOp PkgLength(21) "PFXG" flags 0
        0xa4, 0x94,                                   //   ReturnOp LGreaterOp
        0x11, 0x05, 0x0a, 0x02, 0x01, 0x02,           //     BufferOp PkgLength(5) 2 { 1, 2 }
        0x11, 0x06, 0x0a, 0x03, 0x01, 0x02, 0x03,     //     BufferOp PkgLength(6) 3 { 1, 2, 3 }
        0x14, 0x15, b'B', b'Y', b'T', b'G', 0x00,     // MethodOp PkgLength(21) "BYTG" flags 0
        0xa4, 0x94,                                   //   ReturnOp LGreaterOp
        0x11, 0x05, 0x0a, 0x02, 0x01, 0x03,           //     BufferOp PkgLength(5) 2 { 1, 3 }
        0x11, 0x06, 0x0a, 0x03, 0x01, 0x02, 0x09,     //     BufferOp PkgLength(6) 3 { 1, 2, 9 }
    ];
    let (s, f) = both(&aml, &["\\PFXL", "\\PFXG", "\\BYTG"]);
    assert_eq!(f.results, vec![ok(TRUE), ok(FALSE), ok(TRUE)]);
    assert_eq!(s.results, vec![ok(TRUE), ok(FALSE), ok(FALSE)]);
}

/// ```text
/// Method (STRL, 0) { Return (LLess ("AB", "B")) }
/// Method (BSNL, 0) { Return (LEqual (Buffer (3) { 'A', 'B', 0 }, "AB")) }
/// ```
/// Strings compare byte by byte too ("AB" < "B"; stock's 4-byte Integers say 0x4241 > 0x42). A
/// String right operand of a Buffer comparison is converted with its NUL (ACPICA `exconvrt.c`
/// `AcpiExConvertToBuffer`, length + 1), so it equals the 3-byte Buffer; stock requires a Buffer
/// and panics.
#[test]
fn strings_order_lexicographically_and_convert_with_their_nul() {
    #[rustfmt::skip]
    let aml = [
        0x14, 0x0f, b'S', b'T', b'R', b'L', 0x00,     // MethodOp PkgLength(15) "STRL" flags 0
        0xa4, 0x95,                                   //   ReturnOp LLessOp
        0x0d, b'A', b'B', 0x00,                       //     StringPrefix "AB"
        0x0d, b'B', 0x00,                             //     StringPrefix "B"
        0x14, 0x13, b'B', b'S', b'N', b'L', 0x00,     // MethodOp PkgLength(19) "BSNL" flags 0
        0xa4, 0x93,                                   //   ReturnOp LEqualOp
        0x11, 0x06, 0x0a, 0x03, b'A', b'B', 0x00,     //     BufferOp PkgLength(6) 3 { 'A', 'B', 0 }
        0x0d, b'A', b'B', 0x00,                       //     StringPrefix "AB"
    ];
    let (s, f) = both(&aml, &["\\STRL", "\\BSNL"]);
    assert_eq!(f.results, vec![ok(TRUE), ok(TRUE)]);
    assert_eq!(s.results[0], ok(FALSE));
    assert!(
        matches!(&s.results[1], Err(e) if e.starts_with("panic: ")),
        "{s:?}"
    );
}

// ---- Item 1, review (Chad, relay #48): the right operand converts to the left's type ----
//
// Every expected answer here was also checked on ACPICA's own interpreter: `acpiexec` built from
// the same e85aa3be checkout, run on each program wrapped in a DSDT (`acpiexec -b "execute
// MIXI"`), returns the same TRUE/FALSE or error. In a revision-1 table ACPICA's TRUE is
// 0xFFFFFFFF; the crate's is 64-bit Ones (the known gap in the Fix 3 findings).

/// Stock requires the operand type it expects and fails closed, without a panic.
fn stock_requires_the_same_type(s: &Kat) {
    for r in &s.results {
        assert!(
            matches!(r, Err(e) if e.starts_with("ObjectNotOfExpectedType")),
            "{s:?}"
        );
    }
}

/// ```text
/// Method (MIXI, 0) { Return (LEqual (One, Buffer (One) { 1 })) }
/// ```
/// Chad's input. ACPICA `AcpiExDoLogicalOp` (`exmisc.c:470-487`) converts the right operand to
/// the left's type: a Buffer to an Integer by `AcpiExConvertToInteger`, its first bytes
/// little-endian (`exconvrt.c:123-152`), so `1 == 1`. Fix 3 as merged required an Integer.
#[test]
fn integer_left_buffer_right_converts_the_buffer() {
    #[rustfmt::skip]
    let aml = [
        0x14, 0x0d, b'M', b'I', b'X', b'I', 0x00,     // MethodOp PkgLength(13) "MIXI" flags 0
        0xa4, 0x93, 0x01,                             //   ReturnOp LEqualOp One
        0x11, 0x03, 0x01, 0x01,                       //     BufferOp PkgLength(3) One { 1 }
        0x14, 0x1e, b'I', b'B', b'9', b'L', 0x00,     // MethodOp PkgLength(30) "IB9L" flags 0
        0xa4, 0x93,                                   //   ReturnOp LEqualOp
        0x0e, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, // QWordPrefix 0x0807060504030201
        0x11, 0x0c, 0x0a, 0x09,                       //     BufferOp PkgLength(12) BytePrefix 9
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09,
        0x14, 0x0c, b'I', b'E', b'B', b'0', 0x00,     // MethodOp PkgLength(12) "IEB0" flags 0
        0xa4, 0x93, 0x00,                             //   ReturnOp LEqualOp Zero
        0x11, 0x02, 0x00,                             //     BufferOp PkgLength(2) Zero {}
    ];
    let (s, f) = both(&aml, &["\\MIXI", "\\IB9L", "\\IEB0"]);
    // A 9-byte Buffer gives its first 8 bytes. A zero-length Buffer is an error
    // (`exconvrt.c:127-130`, AE_AML_BUFFER_LIMIT).
    assert_eq!(f.results[..2], [ok(TRUE), ok(TRUE)]);
    assert!(
        matches!(&f.results[2], Err(e) if !e.starts_with("panic: ")),
        "{f:?}"
    );
    stock_requires_the_same_type(&s);
}

/// ```text
/// Method (IS01, 0) { Return (LEqual (One, "1")) }
/// Method (IS10, 0) { Return (LEqual (0x10, "10")) }
/// Method (ISWS, 0) { Return (LEqual (0x1A, " \t0x001a")) }
/// Method (ISTR, 0) { Return (LEqual (0x12, "12G4")) }
/// Method (ISOV, 0) { Return (LEqual (0xFFFFFFFFFFFFFFFF, "FFFFFFFFFFFFFFFFF")) }
/// Method (ISEM, 0) { Return (LEqual (Zero, "")) }
/// Method (IS32, 0) { Return (LEqual (0x1FFFFFFF, "1FFFFFFFF")) }
/// ```
/// A String right operand of an Integer compare is read as hex by `AcpiUtImplicitStrtoul64`
/// (`exconvrt.c:113-116`, `utstrtoul64.c:229-259`): "10" is 0x10, not 10; leading whitespace,
/// a `0x` prefix and leading zeros are skipped, and lower case is accepted; the first non-hex
/// character ends it ("12G4" is 0x12); a digit past the integer width is dropped (17 `F`s are
/// Ones; in a revision-1 table "1FFFFFFFF" is 0x1FFFFFFF); "" is 0. No case is an error.
#[test]
fn integer_left_string_right_reads_the_string_as_hex() {
    #[rustfmt::skip]
    let aml = [
        0x14, 0x0c, b'I', b'S', b'0', b'1', 0x00,     // MethodOp PkgLength(12) "IS01" flags 0
        0xa4, 0x93, 0x01,                             //   ReturnOp LEqualOp One
        0x0d, b'1', 0x00,                             //     StringPrefix "1"
        0x14, 0x0e, b'I', b'S', b'1', b'0', 0x00,     // MethodOp PkgLength(14) "IS10" flags 0
        0xa4, 0x93, 0x0a, 0x10,                       //   ReturnOp LEqualOp BytePrefix 0x10
        0x0d, b'1', b'0', 0x00,                       //     StringPrefix "10"
        0x14, 0x14, b'I', b'S', b'W', b'S', 0x00,     // MethodOp PkgLength(20) "ISWS" flags 0
        0xa4, 0x93, 0x0a, 0x1a,                       //   ReturnOp LEqualOp BytePrefix 0x1A
        0x0d, b' ', 0x09, b'0', b'x', b'0', b'0', b'1', b'a', 0x00, // StringPrefix " \t0x001a"
        0x14, 0x10, b'I', b'S', b'T', b'R', 0x00,     // MethodOp PkgLength(16) "ISTR" flags 0
        0xa4, 0x93, 0x0a, 0x12,                       //   ReturnOp LEqualOp BytePrefix 0x12
        0x0d, b'1', b'2', b'G', b'4', 0x00,           //     StringPrefix "12G4"
        0x14, 0x24, b'I', b'S', b'O', b'V', 0x00,     // MethodOp PkgLength(36) "ISOV" flags 0
        0xa4, 0x93,                                   //   ReturnOp LEqualOp
        0x0e, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, // QWordPrefix 0xFFFFFFFFFFFFFFFF
        0x0d, b'F', b'F', b'F', b'F', b'F', b'F', b'F', b'F', // StringPrefix 17 x "F"
        b'F', b'F', b'F', b'F', b'F', b'F', b'F', b'F', b'F', 0x00,
        0x14, 0x0b, b'I', b'S', b'E', b'M', 0x00,     // MethodOp PkgLength(11) "ISEM" flags 0
        0xa4, 0x93, 0x00,                             //   ReturnOp LEqualOp Zero
        0x0d, 0x00,                                   //     StringPrefix ""
        0x14, 0x18, b'I', b'S', b'3', b'2', 0x00,     // MethodOp PkgLength(24) "IS32" flags 0
        0xa4, 0x93,                                   //   ReturnOp LEqualOp
        0x0c, 0xff, 0xff, 0xff, 0x1f,                 //     DWordPrefix 0x1FFFFFFF
        0x0d, b'1', b'F', b'F', b'F', b'F', b'F', b'F', b'F', b'F', 0x00, // StringPrefix "1FFFFFFFF"
    ];
    let calls = [
        "\\IS01", "\\IS10", "\\ISWS", "\\ISTR", "\\ISOV", "\\ISEM", "\\IS32",
    ];
    let (s, f) = both(&aml, &calls);
    assert_eq!(
        f.results,
        vec![ok(TRUE); 6]
            .into_iter()
            .chain([ok(FALSE)])
            .collect::<Vec<_>>()
    );
    stock_requires_the_same_type(&s);
    // Revision 1: 32-bit integers, so the ninth digit is dropped. (`ISOV` there compares
    // 0xFFFFFFFF, the DWord-truncated QWord, with 8 `F`s.)
    let (_, f1) = both_rev(1, &aml, &["\\IS32"]);
    assert_eq!(f1.results, vec![ok(TRUE)]);
}

/// ```text
/// Method (SI64, 0) { Return (LEqual ("000000000000001A", 0x1A)) }
/// Method (SI32, 0) { Return (LEqual ("0000001A", 0x1A)) }
/// Method (SILC, 0) { Return (LEqual ("000000000000001a", 0x1A)) }
/// Method (SIGT, 0) { Return (LGreater ("1A", 0x1A)) }
/// ```
/// An Integer right operand of a String compare becomes two upper-case hex digits per integer
/// byte, leading zeros kept (`exconvrt.c:473-479`, `AcpiExConvertToAscii`, `uthex.c:23-26`):
/// 16 digits in a revision-2 table, 8 in a revision-1 table. Then the Strings compare byte by
/// byte, so "1A" is greater than "000000000000001A".
#[test]
fn string_left_integer_right_converts_to_hex_digits() {
    #[rustfmt::skip]
    let aml = [
        0x14, 0x1c, b'S', b'I', b'6', b'4', 0x00,     // MethodOp PkgLength(28) "SI64" flags 0
        0xa4, 0x93,                                   //   ReturnOp LEqualOp
        0x0d, b'0', b'0', b'0', b'0', b'0', b'0', b'0', b'0', // StringPrefix "000000000000001A"
        b'0', b'0', b'0', b'0', b'0', b'0', b'1', b'A', 0x00,
        0x0a, 0x1a,                                   //     BytePrefix 0x1A
        0x14, 0x14, b'S', b'I', b'3', b'2', 0x00,     // MethodOp PkgLength(20) "SI32" flags 0
        0xa4, 0x93,                                   //   ReturnOp LEqualOp
        0x0d, b'0', b'0', b'0', b'0', b'0', b'0', b'1', b'A', 0x00, // StringPrefix "0000001A"
        0x0a, 0x1a,                                   //     BytePrefix 0x1A
        0x14, 0x1c, b'S', b'I', b'L', b'C', 0x00,     // MethodOp PkgLength(28) "SILC" flags 0
        0xa4, 0x93,                                   //   ReturnOp LEqualOp
        0x0d, b'0', b'0', b'0', b'0', b'0', b'0', b'0', b'0', // StringPrefix "000000000000001a"
        b'0', b'0', b'0', b'0', b'0', b'0', b'1', b'a', 0x00,
        0x0a, 0x1a,                                   //     BytePrefix 0x1A
        0x14, 0x0e, b'S', b'I', b'G', b'T', 0x00,     // MethodOp PkgLength(14) "SIGT" flags 0
        0xa4, 0x94,                                   //   ReturnOp LGreaterOp
        0x0d, b'1', b'A', 0x00,                       //     StringPrefix "1A"
        0x0a, 0x1a,                                   //     BytePrefix 0x1A
    ];
    let calls = ["\\SI64", "\\SI32", "\\SILC", "\\SIGT"];
    let (s, f) = both(&aml, &calls);
    assert_eq!(f.results, vec![ok(TRUE), ok(FALSE), ok(FALSE), ok(TRUE)]);
    stock_requires_the_same_type(&s);
    let (_, f1) = both_rev(1, &aml, &calls);
    assert_eq!(f1.results, vec![ok(FALSE), ok(TRUE), ok(FALSE), ok(TRUE)]);
}

/// ```text
/// Method (SB02, 0) { Return (LEqual ("0x01 0xAB", Buffer (0x02) { 0x01, 0xAB })) }
/// Method (SB00, 0) { Return (LEqual ("", Buffer (Zero) {})) }
/// ```
/// A Buffer right operand of a String compare becomes `0xNN` per byte, upper case, separated
/// by spaces (`exconvrt.c:556-569` and the loop after it); a zero-length Buffer becomes "".
#[test]
fn string_left_buffer_right_converts_to_spaced_hex_bytes() {
    #[rustfmt::skip]
    let aml = [
        0x14, 0x19, b'S', b'B', b'0', b'2', 0x00,     // MethodOp PkgLength(25) "SB02" flags 0
        0xa4, 0x93,                                   //   ReturnOp LEqualOp
        0x0d, b'0', b'x', b'0', b'1', b' ', b'0', b'x', b'A', b'B', 0x00, // StringPrefix "0x01 0xAB"
        0x11, 0x05, 0x0a, 0x02, 0x01, 0xab,           //     BufferOp PkgLength(5) 2 { 0x01, 0xAB }
        0x14, 0x0d, b'S', b'B', b'0', b'0', 0x00,     // MethodOp PkgLength(13) "SB00" flags 0
        0xa4, 0x93,                                   //   ReturnOp LEqualOp
        0x0d, 0x00,                                   //     StringPrefix ""
        0x11, 0x02, 0x00,                             //     BufferOp PkgLength(2) Zero {}
    ];
    let (s, f) = both(&aml, &["\\SB02", "\\SB00"]);
    assert_eq!(f.results, vec![ok(TRUE), ok(TRUE)]);
    stock_requires_the_same_type(&s);
}

/// ```text
/// Method (IPK1, 0) { Return (LEqual (One, Package (One) { One })) }
/// Method (SPK1, 0) { Return (LEqual ("1", Package (One) { One })) }
/// ```
/// A Package is not a compare operand: ACPICA rejects it at operand resolution
/// (`acopcode.h:232` `ARGI_COMPUTEDATA`; `exresop.c:483-501`, AE_AML_OPERAND_TYPE). It stays
/// an error, not a panic.
#[test]
fn package_right_operand_is_an_error() {
    #[rustfmt::skip]
    let aml = [
        0x14, 0x0d, b'I', b'P', b'K', b'1', 0x00,     // MethodOp PkgLength(13) "IPK1" flags 0
        0xa4, 0x93, 0x01,                             //   ReturnOp LEqualOp One
        0x12, 0x03, 0x01, 0x01,                       //     PackageOp PkgLength(3) 1 { One }
        0x14, 0x0f, b'S', b'P', b'K', b'1', 0x00,     // MethodOp PkgLength(15) "SPK1" flags 0
        0xa4, 0x93,                                   //   ReturnOp LEqualOp
        0x0d, b'1', 0x00,                             //     StringPrefix "1"
        0x12, 0x03, 0x01, 0x01,                       //     PackageOp PkgLength(3) 1 { One }
    ];
    let (s, f) = both(&aml, &["\\IPK1", "\\SPK1"]);
    for r in &f.results {
        assert!(matches!(r, Err(e) if !e.starts_with("panic: ")), "{f:?}");
    }
    stock_requires_the_same_type(&s);
}

/// ```text
/// Method (LA0S, 0) { Return (LAnd (One, "0")) }
/// Method (LO0S, 0) { Return (LOr (Zero, "0")) }
/// Method (LA1S, 0) { Return (LAnd (One, " 0x1")) }
/// Method (LAEB, 0) { Return (LAnd (One, Buffer (Zero) {})) }
/// ```
/// `LAnd`/`LOr` operands are `ARGI_INTEGER` (`acopcode.h:231,249`), converted by the implicit
/// `AcpiExConvertToInteger` (`exresop.c:398-407`): "0" is hex 0, so both are false, " 0x1" is 1,
/// and a zero-length Buffer is an error (AE_AML_BUFFER_LIMIT). Stock requires Integers and fails
/// closed; Fix 3 as merged read a String's first 4 raw bytes ("0" is 0x30, so true) and a
/// zero-length Buffer as 0, a wrong value where stock gave an error.
#[test]
fn land_lor_read_a_string_operand_as_hex() {
    #[rustfmt::skip]
    let aml = [
        0x14, 0x0c, b'L', b'A', b'0', b'S', 0x00,     // MethodOp PkgLength(12) "LA0S" flags 0
        0xa4, 0x90, 0x01,                             //   ReturnOp LAndOp One
        0x0d, b'0', 0x00,                             //     StringPrefix "0"
        0x14, 0x0c, b'L', b'O', b'0', b'S', 0x00,     // MethodOp PkgLength(12) "LO0S" flags 0
        0xa4, 0x91, 0x00,                             //   ReturnOp LOrOp Zero
        0x0d, b'0', 0x00,                             //     StringPrefix "0"
        0x14, 0x0f, b'L', b'A', b'1', b'S', 0x00,     // MethodOp PkgLength(15) "LA1S" flags 0
        0xa4, 0x90, 0x01,                             //   ReturnOp LAndOp One
        0x0d, b' ', b'0', b'x', b'1', 0x00,           //     StringPrefix " 0x1"
        0x14, 0x0c, b'L', b'A', b'E', b'B', 0x00,     // MethodOp PkgLength(12) "LAEB" flags 0
        0xa4, 0x90, 0x01,                             //   ReturnOp LAndOp One
        0x11, 0x02, 0x00,                             //     BufferOp PkgLength(2) Zero {}
    ];
    let (s, f) = both(&aml, &["\\LA0S", "\\LO0S", "\\LA1S", "\\LAEB"]);
    assert_eq!(f.results[..3], [ok(FALSE), ok(FALSE), ok(TRUE)]);
    assert!(
        matches!(&f.results[3], Err(e) if !e.starts_with("panic: ")),
        "{f:?}"
    );
    stock_requires_the_same_type(&s);
}

// ---- Item 2: GenericSerialBus Connection descriptor, I2C type ----------------

/// Layout: ACPICA `amlresrc.h` `AML_RESOURCE_SERIAL_COMMON` + `AML_RESOURCE_I2C_SERIALBUS`; flag
/// meanings: `rsserial.c` `AcpiRsConvertI2cSerialBus` (ACPI 6.5 §6.4.3.8.2). This one sets every
/// flag the decoder reads and carries two vendor bytes, so each field is pinned:
/// device-initiated, consumer, shared, 10-bit, 100 kHz, address 0x2A, vendor `{0xAB, 0xCD}`.
/// ACPICA `iasl -d`, given these bytes in a DSDT `Name (RES0, Buffer …)`, decodes the same:
/// `I2cSerialBusV2 (0x002A, DeviceInitiated, 0x000186A0, AddressingMode10Bit, "\\I2C", 0x07,
/// ResourceConsumer, , Shared, RawDataBuffer (0x02) { 0xAB, 0xCD })`.
#[test]
fn i2c_serial_bus_descriptor_with_vendor_data() {
    #[rustfmt::skip]
    let crs = [
        0x8e, 0x16, 0x00, // Large item 0x0E, length 0x0016 (22)
        0x01,                                         //   revision ID
        0x07,                                         //   resource source index
        0x01,                                         //   serial bus type: I2C
        0x07,                                         //   general flags: device-initiated | consumer | shared
        0x01, 0x00,                                   //   type-specific flags: 10-bit addressing
        0x01,                                         //   type-specific revision
        0x08, 0x00,                                   //   type data length 8 = 6 + 2 vendor bytes
        0xa0, 0x86, 0x01, 0x00,                       //   connection speed 0x000186A0 = 100000
        0x2a, 0x00,                                   //   slave address 0x002A
        0xab, 0xcd,                                   //   vendor data
        b'\\', b'I', b'2', b'C', 0x00,                //   resource source "\I2C"
        0x79, 0x00,                                   // End Tag
    ];
    assert_eq!(
        fix1::decode_resources_kat(&crs),
        Ok(r#"[I2cSerialBus(I2cSerialBusDescriptor { revision_id: 1, resource_source_index: 7, device_initiated: true, is_consumer: true, is_shared: true, ten_bit_addressing: true, type_revision_id: 1, connection_speed: 100000, slave_address: 42, vendor_data: [171, 205], resource_source: "\\I2C" })]"#.to_string())
    );
    assert!(
        matches!(stock::decode_resources_kat(&crs), Err(e) if e.starts_with("panic: ")),
        "stock panics on every GenericSerialBus descriptor"
    );
}

/// The other serial bus types stay fail-closed, and a descriptor too short for its type data or
/// without its NUL-terminated resource source is rejected.
#[test]
fn serial_bus_descriptors_that_are_not_decoded() {
    #[rustfmt::skip]
    let spi = [
        0x8e, 0x14, 0x00, 0x01, 0x00, 0x02, 0x02, 0x00, 0x00, 0x01, // length 0x14; type 2: SPI
        0x09, 0x00,                                   //   type data length 9 (SPI minimum)
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        b'\\', 0x00,                                  //   resource source "\"
        0x79, 0x00,
    ];
    #[rustfmt::skip]
    let short = [
        0x8e, 0x10, 0x00, 0x01, 0x00, 0x01, 0x02, 0x00, 0x00, 0x01, // length 0x10; type 1: I2C
        0x04, 0x00,                                   //   type data length 4 (< 6)
        0x80, 0x1a, 0x06, 0x00,
        b'\\', b'X', 0x00,
        0x79, 0x00,
    ];
    #[rustfmt::skip]
    let no_nul = [
        0x8e, 0x11, 0x00, 0x01, 0x00, 0x01, 0x02, 0x00, 0x00, 0x01, // length 0x11; type 1: I2C
        0x06, 0x00,
        0x80, 0x1a, 0x06, 0x00, 0x15, 0x00,
        b'\\', b'X',                                  //   resource source without its NUL
        0x79, 0x00,
    ];
    assert_eq!(
        fix1::decode_resources_kat(&spi),
        Err("LibUnimplemented".to_string())
    );
    assert_eq!(
        fix1::decode_resources_kat(&short),
        Err("InvalidResourceDescriptor".to_string())
    );
    assert_eq!(
        fix1::decode_resources_kat(&no_nul),
        Err("InvalidResourceDescriptor".to_string())
    );
}

// ---- Item 9: the length of a Buffer read from a field -------------------------

/// ```text
/// OperationRegion (REG0, SystemMemory, Zero, 0x10)
/// Field (REG0, ByteAcc, NoLock, Preserve) { FLD0, 72 }
/// Method (MAIN, 0) { Return (FLD0) }
/// ```
/// A 72-bit field reads as a Buffer of ceil(72/8) = 9 bytes (ACPICA `exfield.c`
/// `AcpiExReadDataFromField`, `ACPI_ROUND_BITS_UP_TO_BYTES`). Stock returned 72 bytes, the bit
/// count rounded to 8 used as a byte count (Fix 2 findings §4). The recording handler reads zero.
#[test]
fn field_read_buffer_is_ceil_bits_over_8_bytes() {
    #[rustfmt::skip]
    let aml = [
        0x5b, 0x80, b'R', b'E', b'G', b'0',           // OpRegionOp "REG0"
        0x00, 0x00, 0x0a, 0x10,                       //   SystemMemory, Zero, BytePrefix 0x10
        0x5b, 0x81, 0x0c, b'R', b'E', b'G', b'0',     // FieldOp PkgLength(12) "REG0"
        0x01,                                         //   FieldFlags: ByteAcc, NoLock, Preserve
        b'F', b'L', b'D', b'0', 0x48, 0x04,           //   NamedField "FLD0" PkgLength 72 (2-byte form)
        0x14, 0x0b, b'M', b'A', b'I', b'N', 0x00,     // MethodOp PkgLength(11) "MAIN" flags 0
        0xa4, b'F', b'L', b'D', b'0',                 //   ReturnOp "FLD0"
    ];
    let (s, f) = both(&aml, &["\\MAIN"]);
    assert_eq!(f.results, vec![ok(&format!("Buffer({})", "00".repeat(9)))]);
    assert_eq!(s.results, vec![ok(&format!("Buffer({})", "00".repeat(72)))]);
}

// ---- Item 7: Acquire returns its result --------------------------------------

/// ```text
/// Mutex (MTX0, 0)
/// Method (MAIN, 0) { Local0 = Acquire (MTX0, 0xFFFF)
///                    Return (Local0) }
/// ```
/// `Acquire` returns True (Ones) on timeout and False (Zero) when it gets the mutex (ACPI 6.5
/// §19.6 `Acquire`; ACPICA `exoparg2.c` turns AE_TIME into a TRUE result). The grade's handler
/// always grants the mutex, so this pins the acquired case; stock contributed no value, so
/// `Local0` stayed Uninitialized.
#[test]
fn acquire_returns_zero_when_acquired() {
    #[rustfmt::skip]
    let aml = [
        0x5b, 0x01, b'M', b'T', b'X', b'0', 0x00,     // MutexOp "MTX0" SyncFlags 0
        0x14, 0x12, b'M', b'A', b'I', b'N', 0x00,     // MethodOp PkgLength(18) "MAIN" flags 0
        0x70, 0x5b, 0x23, b'M', b'T', b'X', b'0',     //   StoreOp AcquireOp "MTX0"
        0xff, 0xff,                                   //     Timeout 0xFFFF
        0x60,                                         //   Local0
        0xa4, 0x60,                                   //   ReturnOp Local0
    ];
    let (s, f) = both(&aml, &["\\MAIN"]);
    assert_eq!(f.results, vec![ok(FALSE)]);
    assert_eq!(s.results, vec![ok("[Uninitialized]")]);
}

// ---- Items 5 and 6: IndexField and BankField ---------------------------------

/// SystemIO accesses as (write, port, width, value), in order.
fn io(k: &Kat) -> Vec<(bool, u64, u8, u64)> {
    k.accesses
        .iter()
        .filter(|a| a.space == acpi_aml_grade::recorder::Space::SystemIo)
        .map(|a| (a.write, a.addr, a.width, a.value))
        .collect()
}

/// ```text
/// OperationRegion (SIO0, SystemIO, 0x2E, 2)
/// Field (SIO0, ByteAcc, NoLock, Preserve) { INDX, 8, DATA, 8 }
/// IndexField (INDX, DATA, ByteAcc, NoLock, Preserve) { Offset (4), CR04, 16 }
/// Method (WR16, 0) { CR04 = 0xBEEF }
/// Method (RD16, 0) { Return (CR04) }
/// ```
/// ACPICA `exfldio.c` `AcpiExFieldDatumIo`: each datum writes its byte offset to the index
/// register and then accesses the data register. `exprep.c` starts the offsets at the field's
/// byte position rounded down to the access width, here 4, so the 16-bit field is two byte
/// datums at index 4 and 5. The recording handler keeps the last value written to a port, so the
/// read returns 0xBE twice, assembled low byte first: 0xBEBE.
#[test]
fn index_field_writes_the_index_then_accesses_the_data_register() {
    #[rustfmt::skip]
    let aml = [
        0x5b, 0x80, b'S', b'I', b'O', b'0', 0x01,     // OpRegionOp "SIO0" SystemIO
        0x0a, 0x2e, 0x0a, 0x02,                       //   BytePrefix 0x2E BytePrefix 2
        0x5b, 0x81, 0x10, b'S', b'I', b'O', b'0', 0x01, // FieldOp PkgLength(16) "SIO0" ByteAcc
        b'I', b'N', b'D', b'X', 0x08,                 //   "INDX" 8 bits
        b'D', b'A', b'T', b'A', 0x08,                 //   "DATA" 8 bits
        0x5b, 0x86, 0x11, b'I', b'N', b'D', b'X',     // IndexFieldOp PkgLength(17) "INDX"
        b'D', b'A', b'T', b'A', 0x01,                 //   "DATA" ByteAcc
        0x00, 0x20,                                   //   ReservedField 32 bits (Offset (4))
        b'C', b'R', b'0', b'4', 0x10,                 //   "CR04" 16 bits
        0x14, 0x0e, b'W', b'R', b'1', b'6', 0x00,     // MethodOp PkgLength(14) "WR16" flags 0
        0x70, 0x0b, 0xef, 0xbe, b'C', b'R', b'0', b'4', //   StoreOp WordPrefix 0xBEEF "CR04"
        0x14, 0x0b, b'R', b'D', b'1', b'6', 0x00,     // MethodOp PkgLength(11) "RD16" flags 0
        0xa4, b'C', b'R', b'0', b'4',                 //   ReturnOp "CR04"
    ];
    let (s, f) = both(&aml, &["\\WR16", "\\RD16"]);
    assert_eq!(f.results[1], ok("Integer(48830)"));
    assert_eq!(
        io(&f),
        vec![
            (true, 0x2e, 1, 4),
            (true, 0x2f, 1, 0xef),
            (true, 0x2e, 1, 5),
            (true, 0x2f, 1, 0xbe),
            (true, 0x2e, 1, 4),
            (false, 0x2f, 1, 0xbe),
            (true, 0x2e, 1, 5),
            (false, 0x2f, 1, 0xbe),
        ]
    );
    for r in &s.results {
        assert!(matches!(r, Err(e) if e.starts_with("panic: ")), "{s:?}");
    }
}

/// ```text
/// OperationRegion (BNKR, SystemIO, 0x50, 1)
/// Field (BNKR, ByteAcc, NoLock, Preserve) { BNK0, 8 }
/// OperationRegion (DREG, SystemIO, 0x60, 4)
/// BankField (DREG, BNK0, 0x02, ByteAcc, NoLock, Preserve) { Offset (1), BF01, 8 }
/// Method (RDBK, 0) { Return (BF01) }
/// ```
/// ACPICA `AcpiExFieldDatumIo`: a BankField writes its bank value to the bank register, then
/// accesses its region as a plain field (here port 0x61).
#[test]
fn bank_field_selects_the_bank_then_accesses_the_region() {
    #[rustfmt::skip]
    let aml = [
        0x5b, 0x80, b'B', b'N', b'K', b'R', 0x01,     // OpRegionOp "BNKR" SystemIO
        0x0a, 0x50, 0x01,                             //   BytePrefix 0x50 One
        0x5b, 0x81, 0x0b, b'B', b'N', b'K', b'R', 0x01, // FieldOp PkgLength(11) "BNKR" ByteAcc
        b'B', b'N', b'K', b'0', 0x08,                 //   "BNK0" 8 bits
        0x5b, 0x80, b'D', b'R', b'E', b'G', 0x01,     // OpRegionOp "DREG" SystemIO
        0x0a, 0x60, 0x0a, 0x04,                       //   BytePrefix 0x60 BytePrefix 4
        0x5b, 0x87, 0x13, b'D', b'R', b'E', b'G',     // BankFieldOp PkgLength(19) "DREG"
        b'B', b'N', b'K', b'0', 0x0a, 0x02, 0x01,     //   "BNK0" BankValue BytePrefix 2, ByteAcc
        0x00, 0x08,                                   //   ReservedField 8 bits (Offset (1))
        b'B', b'F', b'0', b'1', 0x08,                 //   "BF01" 8 bits
        0x14, 0x0b, b'R', b'D', b'B', b'K', 0x00,     // MethodOp PkgLength(11) "RDBK" flags 0
        0xa4, b'B', b'F', b'0', b'1',                 //   ReturnOp "BF01"
    ];
    let (s, f) = both(&aml, &["\\RDBK"]);
    assert_eq!(f.results, vec![ok(FALSE)]);
    assert_eq!(io(&f), vec![(true, 0x50, 1, 2), (false, 0x61, 1, 0)]);
    assert!(
        matches!(&s.results[0], Err(e) if e.starts_with("panic: ")),
        "{s:?}"
    );
}

/// ```text
/// IndexField (INDX, DATA, ByteAcc, NoLock, Preserve) { Offset (0x100), CRXX, 8 }  // index 256
/// Method (RDXX, 0) { Return (CRXX) }
/// ```
/// An index that does not fit the 8-bit index register is an error before any access (ACPICA
/// `AcpiExRegisterOverflow`, AE_AML_REGISTER_LIMIT).
#[test]
fn index_beyond_the_index_register_is_an_error() {
    #[rustfmt::skip]
    let aml = [
        0x5b, 0x80, b'S', b'I', b'O', b'0', 0x01,     // OpRegionOp "SIO0" SystemIO
        0x0a, 0x2e, 0x0a, 0x02,                       //   BytePrefix 0x2E BytePrefix 2
        0x5b, 0x81, 0x10, b'S', b'I', b'O', b'0', 0x01, // FieldOp PkgLength(16) "SIO0" ByteAcc
        b'I', b'N', b'D', b'X', 0x08,                 //   "INDX" 8 bits
        b'D', b'A', b'T', b'A', 0x08,                 //   "DATA" 8 bits
        0x5b, 0x86, 0x12, b'I', b'N', b'D', b'X',     // IndexFieldOp PkgLength(18) "INDX"
        b'D', b'A', b'T', b'A', 0x01,                 //   "DATA" ByteAcc
        0x00, 0x40, 0x80,                             //   ReservedField 0x800 bits (2-byte PkgLength)
        b'C', b'R', b'X', b'X', 0x08,                 //   "CRXX" 8 bits
        0x14, 0x0b, b'R', b'D', b'X', b'X', 0x00,     // MethodOp PkgLength(11) "RDXX" flags 0
        0xa4, b'C', b'R', b'X', b'X',                 //   ReturnOp "CRXX"
    ];
    let (_, f) = both(&aml, &["\\RDXX"]);
    assert_eq!(
        f.results,
        vec![Err(
            "MalformedAml(\"IndexField offset beyond its index register\")".to_string()
        )]
    );
    assert!(io(&f).is_empty(), "{f:?}");
}

// ---- Item 3: GPIO Connection descriptor ---------------------------------------

/// `\_SB.G_IO`'s `GP00` template from the Zenbook S14 DSDT
/// (`fixtures/acpi/zenbook-s14-ux5406aa-303/dsdt.dat` offset 0x12D62, byte for byte):
/// `GpioIo (Shared, PullDefault, 0x0000, 0x0000, IoRestrictionNone, "\\_SB.GPI0", 0x00,
/// ResourceConsumer, _Y32) { 0x0000 }`, as `iasl -d` disassembles it. Layout: ACPICA `amlresrc.h`
/// `AML_RESOURCE_GPIO`; field meanings `rsserial.c` `AcpiRsConvertGpio` (ACPI 6.5 §6.4.3.8.1).
#[test]
fn gpio_io_descriptor_from_the_zenbook_dsdt() {
    #[rustfmt::skip]
    let crs = [
        0x8c, 0x20, 0x00,                             // Large item 0x0C, length 0x0020
        0x01,                                         //   revision ID
        0x01,                                         //   connection type: I/O
        0x01, 0x00,                                   //   general flags: consumer
        0x08, 0x00,                                   //   interrupt/IO flags: shared, IoRestrictionNone
        0x00,                                         //   pin configuration: PullDefault
        0x00, 0x00,                                   //   output drive strength
        0x00, 0x00,                                   //   debounce timeout
        0x17, 0x00,                                   //   pin table offset 23
        0x00,                                         //   resource source index
        0x19, 0x00,                                   //   resource source offset 25
        0x23, 0x00,                                   //   vendor data offset 35 (the end)
        0x00, 0x00,                                   //   vendor data length 0
        0x00, 0x00,                                   //   pin 0
        b'\\', b'_', b'S', b'B', b'.', b'G', b'P', b'I', b'0', 0x00, // "\_SB.GPI0"
        0x79, 0x00,                                   // End Tag
    ];
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/acpi/zenbook-s14-ux5406aa-303/dsdt.dat");
    if path.exists() {
        let dsdt = std::fs::read(&path).unwrap();
        assert_eq!(
            &dsdt[0x12d62..0x12d62 + crs.len()],
            &crs[..],
            "the KAT is the fixture's bytes"
        );
    } else {
        // Real-machine sets are kept in the private repo only; the decode
        // below still runs on the hand-laid bytes.
        eprintln!(
            "note: {} absent, fixture cross-check skipped",
            path.display()
        );
    }
    assert_eq!(
        fix1::decode_resources_kat(&crs),
        Ok(r#"[Gpio(GpioDescriptor { revision_id: 1, connection: Io { io_restriction: 0 }, is_consumer: true, is_shared: true, is_wake_capable: false, pin_config: 0, drive_strength: 0, debounce_timeout: 0, pins: [0], resource_source_index: 0, resource_source: "\\_SB.GPI0", vendor_data: [] })]"#.to_string())
    );
    assert!(
        matches!(stock::decode_resources_kat(&crs), Err(e) if e.starts_with("panic: ")),
        "stock panics on every GPIO descriptor"
    );
}

/// A `GpioInt` with every flag set, two pins, vendor data, and a vendor offset that the
/// decoder must use (`iasl -d`: `GpioInt (Edge, ActiveLow, SharedAndWake, PullUp, 0x0032,
/// "\\GP", 0x00, ResourceConsumer, ) { 0x0005, 0x0006 }` with vendor data `{ 0x5A }`), and a
/// descriptor whose pin table runs past its resource source.
#[test]
fn gpio_int_descriptor_and_a_malformed_one() {
    #[rustfmt::skip]
    let crs = [
        0x8c, 0x1d, 0x00, // Large item 0x0C, length 0x001D (29)
        0x01,                                         //   revision ID
        0x00,                                         //   connection type: interrupt
        0x01, 0x00,                                   //   general flags: consumer
        0x1b, 0x00,                                   //   flags: edge | active-low (1 << 1) | shared | wake
        0x01,                                         //   pin configuration: PullUp
        0x00, 0x00,                                   //   output drive strength
        0x32, 0x00,                                   //   debounce timeout 0x32
        0x17, 0x00,                                   //   pin table offset 23
        0x00,                                         //   resource source index
        0x1b, 0x00,                                   //   resource source offset 27
        0x1f, 0x00,                                   //   vendor data offset 31
        0x01, 0x00,                                   //   vendor data length 1
        0x05, 0x00, 0x06, 0x00,                       //   pins 5, 6
        b'\\', b'G', b'P', 0x00,                      //   "\GP"
        0x5a,                                         //   vendor data
        0x79, 0x00,                                   // End Tag
    ];
    assert_eq!(
        fix1::decode_resources_kat(&crs),
        Ok(r#"[Gpio(GpioDescriptor { revision_id: 1, connection: Interrupt { edge_triggered: true, polarity: 1 }, is_consumer: true, is_shared: true, is_wake_capable: true, pin_config: 1, drive_strength: 0, debounce_timeout: 50, pins: [5, 6], resource_source_index: 0, resource_source: "\\GP", vendor_data: [90] })]"#.to_string())
    );
    let mut bad = crs;
    bad[14] = 0x1d; // pin table offset 29: after the resource source at 27
    assert_eq!(
        fix1::decode_resources_kat(&bad),
        Err("InvalidResourceDescriptor".to_string())
    );
}

// ---- Item 10: AccessAs --------------------------------------------------------

/// ```text
/// OperationRegion (REG0, SystemIO, 0x80, 4)
/// Field (REG0, ByteAcc, NoLock, Preserve) { FLB0, 16, AccessAs (WordAcc), FLW0, 16 }
/// Method (RDB, 0) { Return (FLB0) }
/// Method (RDW, 0) { Return (FLW0) }
/// ```
/// `AccessAs` sets the access width of the fields after it (ACPICA `dsfield.c`
/// `AcpiDsGetFieldNames`), so `FLW0` is one 2-byte read at 0x82 while `FLB0`, before it, is two
/// 1-byte reads at 0x80 and 0x81. Stock: `todo!()` while parsing the field list. `iasl -d`
/// decodes the field list as `FLB0, 16, AccessAs (WordAcc, 0x00), FLW0, 16`.
#[test]
fn access_as_sets_the_width_of_the_fields_after_it() {
    #[rustfmt::skip]
    let aml = [
        0x5b, 0x80, b'R', b'E', b'G', b'0', 0x01,     // OpRegionOp "REG0" SystemIO
        0x0a, 0x80, 0x0a, 0x04,                       //   BytePrefix 0x80 BytePrefix 4
        0x5b, 0x81, 0x13, b'R', b'E', b'G', b'0', 0x01, // FieldOp PkgLength(19) "REG0" ByteAcc
        b'F', b'L', b'B', b'0', 0x10,                 //   "FLB0" 16 bits
        0x01, 0x02, 0x00,                             //   AccessField WordAcc, attribute 0
        b'F', b'L', b'W', b'0', 0x10,                 //   "FLW0" 16 bits
        0x14, 0x0b, b'R', b'D', b'B', b'_', 0x00, // MethodOp PkgLength(11) "RDB_" flags 0
        0xa4, b'F', b'L', b'B', b'0',                 //   ReturnOp "FLB0"
        0x14, 0x0b, b'R', b'D', b'W', b'_', 0x00, // MethodOp PkgLength(11) "RDW_" flags 0
        0xa4, b'F', b'L', b'W', b'0',                 //   ReturnOp "FLW0"
    ];
    let (s, f) = both(&aml, &["\\RDB_", "\\RDW_"]);
    assert_eq!(f.load, Ok(()));
    assert_eq!(f.results, vec![ok(FALSE), ok(FALSE)]);
    assert_eq!(
        io(&f),
        vec![
            (false, 0x80, 1, 0),
            (false, 0x81, 1, 0),
            (false, 0x82, 2, 0)
        ]
    );
    assert!(
        matches!(&s.load, Err(e) if e.starts_with("panic: ")),
        "{s:?}"
    );
}

// ---- LNot: one operand (stock defect found during Fix 3) ------------------------

fn both_rev(rev: u8, aml: &[u8], calls: &[&str]) -> (Kat, Kat) {
    let f = facp();
    (
        stock::run_kat(&f, rev, aml, calls),
        fix1::run_kat(&f, rev, aml, calls),
    )
}

/// ```text
/// Method (MAIN, 0) { If (LNot (One)) { Return (One) }
///                    Return (0x02) }
/// ```
/// `DefLNot := LNotOp Operand` (ACPI 6.5 §20.2.5.4; ACPICA `acopcode.h` `ARGP_LNOT_OP
/// ARGP_LIST1 (ARGP_TERMARG)`). Stock parses `LNot` with two operands, so the `If` predicate
/// swallows `Return (One)` and runs it: stock returns 1. The answer is 2.
#[test]
fn lnot_in_a_predicate_takes_one_operand() {
    #[rustfmt::skip]
    let aml = [
        0x14, 0x0f, b'M', b'A', b'I', b'N', 0x00,     // MethodOp PkgLength(15) "MAIN" flags 0
        0xa0, 0x05, 0x92, 0x01,                       //   IfOp PkgLength(5) LNotOp One
        0xa4, 0x01,                                   //     ReturnOp One
        0xa4, 0x0a, 0x02,                             //   ReturnOp BytePrefix 2
    ];
    let (s, f) = both(&aml, &["\\MAIN"]);
    assert_eq!(f.results, vec![ok("Integer(2)")]);
    assert_eq!(s.results, vec![ok("Integer(1)")]);
}

/// ```text
/// Method (STA0, 0) { Local0 = LNot (One)
///                    Local1 = 7
///                    Return (Local0) }
/// Method (STA1, 0) { <the same>; Return (Local1) }
/// ```
/// As an operand of `Store`, the two-operand `LNot` took `Store (7, Local1)` too: stock panics,
/// and Fix 2 reported MalformedAml. Now Local0 = 0 and Local1 = 7.
#[test]
fn lnot_as_an_operand_takes_one_operand() {
    #[rustfmt::skip]
    let aml = [
        0x14, 0x10, b'S', b'T', b'A', b'0', 0x00,     // MethodOp PkgLength(16) "STA0" flags 0
        0x70, 0x92, 0x01, 0x60,                       //   StoreOp LNotOp One Local0
        0x70, 0x0a, 0x07, 0x61,                       //   StoreOp BytePrefix 7 Local1
        0xa4, 0x60,                                   //   ReturnOp Local0
        0x14, 0x10, b'S', b'T', b'A', b'1', 0x00,     // MethodOp PkgLength(16) "STA1" flags 0
        0x70, 0x92, 0x01, 0x60,                       //   StoreOp LNotOp One Local0
        0x70, 0x0a, 0x07, 0x61,                       //   StoreOp BytePrefix 7 Local1
        0xa4, 0x61,                                   //   ReturnOp Local1
    ];
    let (s, f) = both(&aml, &["\\STA0", "\\STA1"]);
    assert_eq!(f.results, vec![ok(FALSE), ok("Integer(7)")]);
    for r in &s.results {
        assert!(matches!(r, Err(e) if e.starts_with("panic: ")), "{s:?}");
    }
}

/// ```text
/// Method (LNZ0, 0) { Return (LNot (Zero)) }
/// ```
/// Ones at the table's integer width: all 64 bits in a revision-2 table, 32 in a revision-1 one
/// (ACPICA `exoparg1.c` returns ACPI_UINT64_MAX; `dswexec.c` truncates operator results in a
/// 32-bit table).
#[test]
fn lnot_of_zero_is_ones_at_the_integer_width() {
    #[rustfmt::skip]
    let aml = [
        0x14, 0x09, b'L', b'N', b'Z', b'0', 0x00,     // MethodOp PkgLength(9) "LNZ0" flags 0
        0xa4, 0x92, 0x00,                             //   ReturnOp LNotOp Zero
    ];
    let (_, f2) = both_rev(2, &aml, &["\\LNZ0"]);
    let (_, f1) = both_rev(1, &aml, &["\\LNZ0"]);
    assert_eq!(f2.results, vec![ok(TRUE)]);
    assert_eq!(f1.results, vec![ok("Integer(4294967295)")]);
}

/// ```text
/// Method (NE12, 0) { Return (LNotEqual (One, 2)) }       // 0x92 0x93
/// Method (LE22, 0) { Return (LLessEqual (2, 2)) }        // 0x92 0x94
/// Method (GE12, 0) { Return (LGreaterEqual (One, 2)) }   // 0x92 0x95
/// ```
/// The 0x92-prefixed pairs keep their two operands (ACPI 6.5 §20.2.5.4: `LNotEqualOp :=
/// LNotOp LEqualOp`, and likewise). Both crates agree.
#[test]
fn lnot_prefixed_comparisons_keep_two_operands() {
    #[rustfmt::skip]
    let aml = [
        0x14, 0x0c, b'N', b'E', b'1', b'2', 0x00,     // MethodOp PkgLength(12) "NE12" flags 0
        0xa4, 0x92, 0x93, 0x01, 0x0a, 0x02,           //   ReturnOp LNotEqualOp One BytePrefix 2
        0x14, 0x0d, b'L', b'E', b'2', b'2', 0x00,     // MethodOp PkgLength(13) "LE22" flags 0
        0xa4, 0x92, 0x94, 0x0a, 0x02, 0x0a, 0x02,     //   ReturnOp LLessEqualOp 2 2
        0x14, 0x0c, b'G', b'E', b'1', b'2', 0x00,     // MethodOp PkgLength(12) "GE12" flags 0
        0xa4, 0x92, 0x95, 0x01, 0x0a, 0x02,           //   ReturnOp LGreaterEqualOp One BytePrefix 2
    ];
    let (s, f) = both(&aml, &["\\NE12", "\\LE22", "\\GE12"]);
    assert_eq!(f.results, vec![ok(TRUE), ok(TRUE), ok(FALSE)]);
    assert_eq!(s.results, f.results);
}

// ---- Item 8: Store to a named Buffer or String ------------------------------

/// ```text
/// Name (BUF1, Buffer (4) { 1, 2, 3, 4 })     Method (SHRT) { BUF1 = Buffer (2) { 0xAA, 0xBB }; Return (BUF1) }
/// Name (BUF2, Buffer (4) { 1, 2, 3, 4 })     Method (LONG) { BUF2 = Buffer (6) { 9, 8, 7, 6, 5, 4 }; Return (BUF2) }
/// Name (BUF3, Buffer (4) { 1, 2, 3, 4 })     Method (INTG) { BUF3 = 0x1122334455667788; Return (BUF3) }
/// Name (BUF4, Buffer (4) { 1, 2, 3, 4 })     Method (STRB) { BUF4 = "AB"; Return (BUF4) }
/// Name (BUFE, Buffer (0) {})                 Method (GROW) { BUFE = Buffer (3) { 1, 2, 3 }; Return (BUFE) }
/// Name (STR0, "ABCD")                        Method (STRS) { STR0 = "XY"; Return (STR0) }
///                                            Method (STRI) { STR0 = 5 }
///                                            Method (SELF) { BUF1 = BUF1; Return (BUF1) }
/// ```
/// ACPICA `exstoren.c` `AcpiExStoreObjectToObject` → `exstorob.c`:
/// - a Buffer target keeps its size: a shorter source is zero-filled, a longer one truncated;
/// - an Integer source contributes its integer-width bytes and a String source its bytes and
///   NUL (`exconvrt.c` `AcpiExConvertToBuffer`);
/// - a zero-length target takes the source's length (`STORE_GROWS_EMPTY_BUFFER`);
/// - a String target takes the String source, at its length;
/// - storing an object into itself changes nothing.
///
/// Converting an Integer to a String on Store is not built and fails closed. Stock panics on
/// every store to a named Buffer or String.
#[test]
fn store_to_a_named_buffer_or_string() {
    #[rustfmt::skip]
    let aml = [
        0x08, b'B', b'U', b'F', b'1', 0x11, 0x07, 0x0a, 0x04, 0x01, 0x02, 0x03, 0x04, // Name BUF1 Buffer(4)
        0x08, b'B', b'U', b'F', b'2', 0x11, 0x07, 0x0a, 0x04, 0x01, 0x02, 0x03, 0x04, // Name BUF2 Buffer(4)
        0x08, b'B', b'U', b'F', b'3', 0x11, 0x07, 0x0a, 0x04, 0x01, 0x02, 0x03, 0x04, // Name BUF3 Buffer(4)
        0x08, b'B', b'U', b'F', b'4', 0x11, 0x07, 0x0a, 0x04, 0x01, 0x02, 0x03, 0x04, // Name BUF4 Buffer(4)
        0x08, b'B', b'U', b'F', b'E', 0x11, 0x03, 0x0a, 0x00,                         // Name BUFE Buffer(0)
        0x08, b'S', b'T', b'R', b'0', 0x0d, b'A', b'B', b'C', b'D', 0x00,             // Name STR0 "ABCD"
        0x14, 0x16, b'S', b'H', b'R', b'T', 0x00,     // MethodOp PkgLength(22) "SHRT"
        0x70, 0x11, 0x05, 0x0a, 0x02, 0xaa, 0xbb,     //   StoreOp Buffer(2) { 0xAA, 0xBB }
        b'B', b'U', b'F', b'1',                       //     "BUF1"
        0xa4, b'B', b'U', b'F', b'1',                 //   ReturnOp "BUF1"
        0x14, 0x1a, b'L', b'O', b'N', b'G', 0x00,     // MethodOp PkgLength(26) "LONG"
        0x70, 0x11, 0x09, 0x0a, 0x06,                 //   StoreOp Buffer(6)
        0x09, 0x08, 0x07, 0x06, 0x05, 0x04,           //     { 9, 8, 7, 6, 5, 4 }
        b'B', b'U', b'F', b'2',                       //     "BUF2"
        0xa4, b'B', b'U', b'F', b'2',                 //   ReturnOp "BUF2"
        0x14, 0x19, b'I', b'N', b'T', b'G', 0x00,     // MethodOp PkgLength(25) "INTG"
        0x70, 0x0e,                                   //   StoreOp QWordPrefix
        0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11, //     0x1122334455667788
        b'B', b'U', b'F', b'3',                       //     "BUF3"
        0xa4, b'B', b'U', b'F', b'3',                 //   ReturnOp "BUF3"
        0x14, 0x14, b'S', b'T', b'R', b'B', 0x00,     // MethodOp PkgLength(20) "STRB"
        0x70, 0x0d, b'A', b'B', 0x00,                 //   StoreOp "AB"
        b'B', b'U', b'F', b'4',                       //     "BUF4"
        0xa4, b'B', b'U', b'F', b'4',                 //   ReturnOp "BUF4"
        0x14, 0x17, b'G', b'R', b'O', b'W', 0x00,     // MethodOp PkgLength(23) "GROW"
        0x70, 0x11, 0x06, 0x0a, 0x03, 0x01, 0x02, 0x03, //   StoreOp Buffer(3) { 1, 2, 3 }
        b'B', b'U', b'F', b'E',                       //     "BUFE"
        0xa4, b'B', b'U', b'F', b'E',                 //   ReturnOp "BUFE"
        0x14, 0x14, b'S', b'T', b'R', b'S', 0x00,     // MethodOp PkgLength(20) "STRS"
        0x70, 0x0d, b'X', b'Y', 0x00,                 //   StoreOp "XY"
        b'S', b'T', b'R', b'0',                       //     "STR0"
        0xa4, b'S', b'T', b'R', b'0',                 //   ReturnOp "STR0"
        0x14, 0x0d, b'S', b'T', b'R', b'I', 0x00,     // MethodOp PkgLength(13) "STRI"
        0x70, 0x0a, 0x05, b'S', b'T', b'R', b'0',     //   StoreOp BytePrefix 5 "STR0"
        0x14, 0x14, b'S', b'E', b'L', b'F', 0x00,     // MethodOp PkgLength(20) "SELF"
        0x70, b'B', b'U', b'F', b'1', b'B', b'U', b'F', b'1', //   StoreOp "BUF1" "BUF1"
        0xa4, b'B', b'U', b'F', b'1',                 //   ReturnOp "BUF1"
    ];
    let calls = [
        "\\SHRT", "\\LONG", "\\INTG", "\\STRB", "\\GROW", "\\STRS", "\\STRI", "\\SELF",
    ];
    let (s, f) = both(&aml, &calls);
    assert_eq!(f.load, Ok(()));
    assert_eq!(
        f.results,
        vec![
            ok("Buffer(aabb0000)"),
            ok("Buffer(09080706)"),
            ok("Buffer(88776655)"),
            ok("Buffer(41420000)"),
            ok("Buffer(010203)"),
            ok("String(\"XY\")"),
            Err("LibUnimplemented".to_string()),
            ok("Buffer(aabb0000)"),
        ]
    );
    assert!(
        matches!(&s.results[0], Err(e) if e.starts_with("panic: ")),
        "{s:?}"
    );
}

/// ```text
/// Name (BUFN, Buffer (0) {})
/// Method (NULB) { BUFN = "AB"; Return (BUFN) }
/// ```
/// A String stored to a Buffer carries its NUL (ACPICA `exconvrt.c` `AcpiExConvertToBuffer`,
/// length + 1). Into a zero-length target, which takes the source's length, that shows as 3
/// bytes, `41 42 00`; into a non-empty target the NUL can't be told from the zero fill.
#[test]
fn string_stored_to_a_buffer_carries_its_nul() {
    #[rustfmt::skip]
    let aml = [
        0x08, b'B', b'U', b'F', b'N', 0x11, 0x03, 0x0a, 0x00, // Name BUFN Buffer(0)
        0x14, 0x14, b'N', b'U', b'L', b'B', 0x00,     // MethodOp PkgLength(20) "NULB"
        0x70, 0x0d, b'A', b'B', 0x00,                 //   StoreOp "AB"
        b'B', b'U', b'F', b'N',                       //     "BUFN"
        0xa4, b'B', b'U', b'F', b'N',                 //   ReturnOp "BUFN"
    ];
    let (_, f) = both(&aml, &["\\NULB"]);
    assert_eq!(f.results, vec![ok("Buffer(414200)")]);
}
