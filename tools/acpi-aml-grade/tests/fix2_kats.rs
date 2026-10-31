// SPDX-License-Identifier: MPL-2.0
//! OT-86 Fix 2 known-answer tests: one hand-laid malformed-AML program per
//! class of panic Fix 2 converts to an error (`PANIC-CENSUS.md` in
//! `third_party/acpi-loganfy`), run through the same recording handler the
//! grade uses (`run_kat`).
//!
//! These are byte oracles in the sense of CLAUDE.md, laid out the way
//! `fix1_kats.rs` is: every program is written by hand from the AML grammar
//! (ACPI 6.5 chapter 20), each byte commented, and the opcode values were read
//! from the raw ACPICA header `source/include/amlcode.h` (acpica/acpica @
//! e85aa3be). The ASL in each doc comment is what the bytes say; where the
//! bytes are malformed on purpose, the comment says which part.
//!
//! Each test pins the Fix 2 error exactly and, where running it is safe, the
//! stock panic that the class is named after. Some stock failures are not
//! catchable in-process (a stack overflow, an allocation failure, a loop that
//! never ends); those tests run only the patched crate, and the stock
//! behaviour is recorded in the Fix 2 findings with the fuzz case that shows
//! it.

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

/// Run `aml` on the patched crate only (the stock outcome is not catchable).
fn patched(aml: &[u8], calls: &[&str]) -> Kat {
    fix1::run_kat(&facp(), 2, aml, calls)
}

fn malformed(check: &str) -> String {
    format!("MalformedAml({check:?})")
}

fn stock_panicked(r: &Result<impl std::fmt::Debug, String>, what: &str) -> bool {
    matches!(r, Err(e) if e.starts_with("panic: ") && e.contains(what))
}

// ---- PkgLength ---------------------------------------------------------------

/// ```text
/// Method (MAIN, 0) {}     // PkgLength says 63 bytes; the table ends after 6
/// ```
/// ACPI 6.5 §20.2.4 PkgLength: the length covers the package, which must lie inside its
/// enclosing block. Stock slices past the end of the table (`mod.rs:1243`).
#[test]
fn pkglength_past_the_end_of_its_block() {
    #[rustfmt::skip]
    let aml = [
        0x14, 0x3f, b'M', b'A', b'I', b'N', 0x00,     // MethodOp PkgLength(63) "MAIN" flags 0; no body
    ];
    let (s, f) = both(&aml, &[]);
    assert_eq!(
        f.load,
        Err(malformed(
            "PkgLength runs past the end of its enclosing block"
        ))
    );
    assert!(stock_panicked(&s.load, "out of range for slice"), "{s:?}");
}

/// ```text
/// Method (MAIN, 0) {}     // PkgLength says 2 bytes; the name and flags alone are 5
/// ```
/// ACPI 6.5 §20.2.4 PkgLength. Stock subtracts the bytes consumed from the length and
/// overflows (`mod.rs:1241`).
#[test]
fn pkglength_shorter_than_its_header() {
    #[rustfmt::skip]
    let aml = [
        0x14, 0x02, b'M', b'A', b'I', b'N', 0x00,     // MethodOp PkgLength(2) "MAIN" flags 0
    ];
    let (s, f) = both(&aml, &[]);
    assert_eq!(
        f.load,
        Err(malformed(
            "PkgLength shorter than the bytes it covers so far"
        ))
    );
    assert!(stock_panicked(&s.load, "subtract with overflow"), "{s:?}");
}

// ---- Buffer ------------------------------------------------------------------

/// ```text
/// Name (BUF0, Buffer (2) { 0x01, 0x02, 0x03 })   // three initializer bytes for a 2-byte Buffer
/// ```
/// ACPI 6.5 §19.6 `Buffer`. Stock copies the initializer into a `BufferSize` vector and
/// panics on the length mismatch (`mod.rs:649`). ACPICA grows the buffer to the initializer
/// instead; Fix 2 fails closed (findings §3).
#[test]
fn buffer_initializer_longer_than_its_size() {
    #[rustfmt::skip]
    let aml = [
        0x08, b'B', b'U', b'F', b'0',                 // NameOp "BUF0"
        0x11, 0x06, 0x0a, 0x02,                       //   BufferOp PkgLength(6) BytePrefix 2
        0x01, 0x02, 0x03,                             //     initializer
    ];
    let (s, f) = both(&aml, &[]);
    assert_eq!(
        f.load,
        Err(malformed("Buffer initializer longer than its BufferSize"))
    );
    assert!(stock_panicked(&s.load, "out of range for slice"), "{s:?}");
}

/// ```text
/// Name (BUF0, Buffer (0xFFFFFFFF) {})
/// ```
/// Stock allocates 4 GiB. Fix 2 caps an AML-sized allocation at `MAX_OBJECT_BYTES` (a policy
/// limit, findings §3). Patched crate only: the stock allocation is not a catchable failure.
#[test]
fn buffer_size_over_the_object_limit() {
    #[rustfmt::skip]
    let aml = [
        0x08, b'B', b'U', b'F', b'0',                 // NameOp "BUF0"
        0x11, 0x06, 0x0c, 0xff, 0xff, 0xff, 0xff,     //   BufferOp PkgLength(6) DWordPrefix 0xFFFFFFFF
    ];
    let f = patched(&aml, &[]);
    assert_eq!(
        f.load,
        Err(malformed("BufferSize over the interpreter's object limit"))
    );
}

// ---- Package / VarPackage ----------------------------------------------------

/// ```text
/// Name (PKG0, Package (1) { One, 2 })   // two initializers for NumElements 1
/// ```
/// ACPI 6.5 §19.6 `Package`. Stock asserts that the block is used up (`mod.rs:677`). ACPICA
/// truncates with a warning; Fix 2 fails closed (findings §3).
#[test]
fn package_with_surplus_initializers() {
    #[rustfmt::skip]
    let aml = [
        0x08, b'P', b'K', b'G', b'0',                 // NameOp "PKG0"
        0x12, 0x05, 0x01,                             //   PackageOp PkgLength(5) NumElements 1
        0x01, 0x0a, 0x02,                             //     One, BytePrefix 2
    ];
    let (s, f) = both(&aml, &[]);
    assert_eq!(
        f.load,
        Err(malformed(
            "Package holds more elements than its NumElements"
        ))
    );
    assert!(
        stock_panicked(&s.load, "assertion `left == right` failed"),
        "{s:?}"
    );
}

/// ```text
/// Name (PKG1, VarPackage () {})   // no VarNumElements at all
/// ```
/// ACPI 6.5 §20.2.5.4 DefVarPackage: `VarPackageOp PkgLength VarNumElements PackageElementList`.
/// Stock indexes the missing count (`mod.rs:1058`).
#[test]
fn varpackage_without_its_count() {
    #[rustfmt::skip]
    let aml = [
        0x08, b'P', b'K', b'G', b'1',                 // NameOp "PKG1"
        0x13, 0x01,                                   //   VarPackageOp PkgLength(1); nothing follows
    ];
    let (s, f) = both(&aml, &[]);
    assert_eq!(f.load, Err(malformed("VarPackage without its NumElements")));
    assert!(stock_panicked(&s.load, "index out of bounds"), "{s:?}");
}

// ---- Create*Field ------------------------------------------------------------

/// ```text
/// Name (BUF0, Buffer (1) { 0x00 })
/// CreateDWordField (BUF0, Zero, DW00)   // 32 bits over an 8-bit buffer
/// Method (MAIN, 0) { DW00 = One }
/// ```
/// ACPI 6.5 §19.6 `CreateDWordField`: the field must lie inside the buffer (ACPICA returns
/// AE_AML_BUFFER_LIMIT, `dsopcode.c` `AcpiDsInitBufferField`). Stock creates it and then indexes past the buffer when it is written
/// (`object.rs:464`).
#[test]
fn create_field_outside_its_buffer() {
    #[rustfmt::skip]
    let aml = [
        0x08, b'B', b'U', b'F', b'0',                 // NameOp "BUF0"
        0x11, 0x04, 0x0a, 0x01, 0x00,                 //   BufferOp PkgLength(4) BytePrefix 1 { 0x00 }
        0x8a, b'B', b'U', b'F', b'0', 0x00,           // CreateDWordFieldOp "BUF0" Zero
        b'D', b'W', b'0', b'0',                       //   "DW00"
        0x14, 0x0c, b'M', b'A', b'I', b'N', 0x00,     // MethodOp PkgLength(12) "MAIN" flags 0
        0x70, 0x01, b'D', b'W', b'0', b'0',           //   StoreOp One "DW00"
    ];
    let (s, f) = both(&aml, &["\\MAIN"]);
    assert_eq!(f.load, Err(malformed("buffer field outside its buffer")));
    assert_eq!(s.load, Ok(()));
    assert!(
        stock_panicked(&s.results[0], "index out of bounds"),
        "{s:?}"
    );
}

// ---- Mid ---------------------------------------------------------------------

/// ```text
/// Method (MAIN, 0) { Return (Mid ("AB", One, 5)) }
/// ```
/// ACPI 6.5 §19.6 `Mid`: when `Index + Length` passes the end, only the bytes that exist are
/// returned (ACPICA `AcpiExOpcode_3A_1T_1R` in `exoparg3.c` clips the length the same way).
/// Stock slices past the string's end (`mod.rs:2082`).
#[test]
fn mid_past_the_end_returns_the_rest() {
    #[rustfmt::skip]
    let aml = [
        0x14, 0x10, b'M', b'A', b'I', b'N', 0x00,     // MethodOp PkgLength(16) "MAIN" flags 0
        0xa4, 0x9e,                                   //   ReturnOp MidOp
        0x0d, b'A', b'B', 0x00,                       //     StringPrefix "AB"
        0x01, 0x0a, 0x05, 0x00,                       //     One, BytePrefix 5, NullName
    ];
    let (s, f) = both(&aml, &["\\MAIN"]);
    assert_eq!(f.results, vec![Ok("String(\"B\")".to_string())]);
    assert!(stock_panicked(&s.results[0], "out of bounds"), "{s:?}");
}

// ---- Divide / Mod ------------------------------------------------------------

/// ```text
/// Method (MAIN, 0) { Return (Divide (One, Zero)) }
/// Method (MODZ, 0) { Return (Mod (One, Zero)) }
/// ```
/// ACPI 6.5 §19.6 `Divide`, `Mod`: a zero divisor is an error (ACPICA returns
/// AE_AML_DIVIDE_BY_ZERO from `utmath.c` `AcpiUtDivide`). Stock panics in the Rust division.
#[test]
fn divide_and_mod_by_zero() {
    #[rustfmt::skip]
    let aml = [
        0x14, 0x0c, b'M', b'A', b'I', b'N', 0x00,     // MethodOp PkgLength(12) "MAIN" flags 0
        0xa4, 0x78, 0x01, 0x00, 0x00, 0x00,           //   ReturnOp DivideOp One Zero NullName NullName
        0x14, 0x0b, b'M', b'O', b'D', b'Z', 0x00,     // MethodOp PkgLength(11) "MODZ" flags 0
        0xa4, 0x85, 0x01, 0x00, 0x00,                 //   ReturnOp ModOp One Zero NullName
    ];
    let (s, f) = both(&aml, &["\\MAIN", "\\MODZ"]);
    let e = Err(malformed("Divide or Mod by zero"));
    assert_eq!(f.results, vec![e.clone(), e]);
    for r in &s.results {
        assert!(stock_panicked(r, "divisor of zero"), "{s:?}");
    }
}

// ---- While -------------------------------------------------------------------

/// ```text
/// Method (MAIN, 0) { While () {} }   // PkgLength(1): no Predicate
/// ```
/// ACPI 6.5 §20.2.5.3 DefWhile: `WhileOp PkgLength Predicate TermList`. Stock re-enters the
/// loop forever, growing its in-flight stack until allocation fails (fuzz case 2953 of seed
/// 0x0786 before the fix). Patched crate only.
#[test]
fn while_without_a_predicate() {
    #[rustfmt::skip]
    let aml = [
        0x14, 0x08, b'M', b'A', b'I', b'N', 0x00,     // MethodOp PkgLength(8) "MAIN" flags 0
        0xa2, 0x01,                                   //   WhileOp PkgLength(1)
    ];
    let f = patched(&aml, &["\\MAIN"]);
    assert_eq!(f.load, Ok(()));
    assert_eq!(f.results, vec![Err(malformed("While without a predicate"))]);
}

// ---- Names -------------------------------------------------------------------

/// ```text
/// Name (\, One)   // the root itself as the new object's name
/// ```
/// ACPI 6.5 §20.2.2 NameString: `RootChar NullName` names the root scope, which already exists
/// and is not an object. Stock asserts (`namespace.rs:311`).
#[test]
fn root_scope_as_an_object_name() {
    #[rustfmt::skip]
    let aml = [
        0x08, 0x5c, 0x00, 0x01,                       // NameOp RootChar NullName One
    ];
    let (s, f) = both(&aml, &[]);
    assert_eq!(
        f.load,
        Err(malformed("the root scope used as an object name"))
    );
    assert!(
        stock_panicked(&s.load, "assertion `left != right` failed"),
        "{s:?}"
    );
}

/// ```text
/// Name (STR0, "\xFF")
/// ```
/// ACPI 6.5 §20.2.3 String: `AsciiCharList` is 0x01-0x7F. Stock unwraps the UTF-8 decode
/// (`mod.rs:1166`).
#[test]
fn string_literal_that_is_not_ascii() {
    #[rustfmt::skip]
    let aml = [
        0x08, b'S', b'T', b'R', b'0',                 // NameOp "STR0"
        0x0d, 0xff, 0x00,                             //   StringPrefix 0xFF NullChar
    ];
    let (s, f) = both(&aml, &[]);
    assert_eq!(
        f.load,
        Err(malformed("String literal that is not ASCII/UTF-8"))
    );
    assert!(stock_panicked(&s.load, "Utf8Error"), "{s:?}");
}

// ---- Reference cycles and nesting --------------------------------------------

/// ```text
/// Method (MAIN, 0) { Store (RefOf (Local0), Local0)
///                    Return (ObjectType (Local0)) }
/// ```
/// Four bytes that make Local0 refer to itself. Stock stores it and then overflows the stack
/// walking it (`typ()` recurses through references), which ends the process, so this runs the
/// patched crate only. Fix 2 refuses the store (findings §3).
#[test]
fn store_that_makes_a_local_refer_to_itself() {
    #[rustfmt::skip]
    let aml = [
        0x14, 0x0d, b'M', b'A', b'I', b'N', 0x00,     // MethodOp PkgLength(13) "MAIN" flags 0
        0x70, 0x71, 0x60, 0x60,                       //   StoreOp RefOfOp Local0 Local0
        0xa4, 0x8e, 0x60,                             //   ReturnOp ObjectTypeOp Local0
    ];
    let f = patched(&aml, &["\\MAIN"]);
    assert_eq!(
        f.results,
        vec![Err(malformed(
            "store that would make an object refer to itself, or nest too deeply"
        ))]
    );
}

/// ```text
/// Name (PKG0, Package (1) { Zero })
/// Method (MAIN, 0) { Store (RefOf (PKG0), Index (PKG0, Zero))
///                    Return (Zero) }
/// ```
/// The same cycle through a package element (ACPI 6.5 §19.6 `Index`). Patched crate only.
#[test]
fn store_that_makes_a_package_contain_itself() {
    #[rustfmt::skip]
    let aml = [
        0x08, b'P', b'K', b'G', b'0',                 // NameOp "PKG0"
        0x12, 0x03, 0x01, 0x00,                       //   PackageOp PkgLength(3) NumElements 1 { Zero }
        0x14, 0x15, b'M', b'A', b'I', b'N', 0x00,     // MethodOp PkgLength(21) "MAIN" flags 0
        0x70, 0x71, b'P', b'K', b'G', b'0',           //   StoreOp RefOfOp "PKG0"
        0x88, b'P', b'K', b'G', b'0', 0x00, 0x00,     //     IndexOp "PKG0" Zero NullName
        0xa4, 0x00,                                   //   ReturnOp Zero
    ];
    let f = patched(&aml, &["\\MAIN"]);
    assert_eq!(f.load, Ok(()));
    assert_eq!(
        f.results,
        vec![Err(malformed(
            "store that would make an object refer to itself, or nest too deeply"
        ))]
    );
}

/// ```text
/// Method (MAIN, 0) { Return (MAIN ()) }
/// ```
/// Unbounded recursion: stock grows its context stack until allocation fails. Fix 2 stops at
/// `MAX_NESTING` (256) method levels, a policy limit recorded in findings §3. ACPICA stops a
/// method at 255 concurrent invocations (AE_AML_METHOD_LIMIT, `dsmethod.c`). Patched crate only.
#[test]
fn method_recursion_is_bounded() {
    #[rustfmt::skip]
    let aml = [
        0x14, 0x0b, b'M', b'A', b'I', b'N', 0x00,     // MethodOp PkgLength(11) "MAIN" flags 0
        0xa4, b'M', b'A', b'I', b'N',                 //   ReturnOp MethodInvocation "MAIN" (no args)
    ];
    let f = patched(&aml, &["\\MAIN"]);
    assert_eq!(
        f.results,
        vec![Err(malformed(
            "method calls nested deeper than the interpreter's limit"
        ))]
    );
}

/// ```text
/// Method (MAIN, 0) { Return (Add (Add (... Add (One, One) ..., One), One)) }   // 300 Adds
/// ```
/// Operator nesting past `MAX_NESTING`. Stock evaluates it (301); Fix 2 stops at the limit. The
/// class this guards is a loop whose body leaves an operator without operands, which stock grew
/// until allocation failed (fuzz case 40222 of seed 0x0786 on 3c7b67af); 300 legal levels is the
/// deterministic way to reach the same check. A policy divergence, recorded in findings §3.
#[test]
fn operator_nesting_is_bounded() {
    const DEPTH: usize = 300;
    // Body: ReturnOp, DEPTH AddOps, One, then DEPTH times (One NullName).
    let body_len = 1 + DEPTH + 1 + 2 * DEPTH;
    // PkgLength covers itself (2 bytes), "MAIN", the flags byte and the body.
    let pkg = 2 + 4 + 1 + body_len;
    assert!(pkg < 1 << 12);
    // ACPI 6.5 §20.2.4: a 2-byte PkgLength is lead byte 0b01xx_nnnn (low nibble) then bits 4-11.
    let mut aml = vec![
        0x14,
        0x40 | (pkg & 0xf) as u8,
        (pkg >> 4) as u8,
        b'M',
        b'A',
        b'I',
        b'N',
        0x00,
    ];
    aml.push(0xa4); // ReturnOp
    aml.extend(std::iter::repeat_n(0x72, DEPTH)); // AddOp ...
    aml.push(0x01); // One (the innermost Addend1)
    for _ in 0..DEPTH {
        aml.extend([0x01, 0x00]); // Addend2 One, Target NullName
    }
    assert_eq!(aml.len(), 1 + pkg);
    let (s, f) = both(&aml, &["\\MAIN"]);
    assert_eq!(s.results, vec![Ok("Integer(301)".to_string())]);
    assert_eq!(
        f.results,
        vec![Err(malformed(
            "operators nested deeper than the interpreter's limit"
        ))]
    );
}

// ---- Fields ------------------------------------------------------------------

/// ```text
/// OperationRegion (REG0, SystemMemory, Zero, 0x10)
/// Field (REG0, ByteAcc, NoLock, 3) { FLD0, 4 }   // UpdateRule 3 is reserved
/// Method (MAIN, 0) { FLD0 = One }
/// ```
/// ACPI 6.5 §20.2.5.2 FieldFlags: bits 5-6 are the UpdateRule and 3 is reserved, but AML can
/// encode it. A 4-bit write under a byte access needs the rule; stock reaches `unreachable!()`
/// (`object.rs:408` in the patched numbering). Fix 2 returns `InvalidFieldFlags`, the error the
/// crate already uses for a reserved AccessType.
#[test]
fn reserved_field_update_rule() {
    #[rustfmt::skip]
    let aml = [
        0x5b, 0x80, b'R', b'E', b'G', b'0',           // OpRegionOp "REG0"
        0x00, 0x00, 0x0a, 0x10,                       //   SystemMemory, Zero, BytePrefix 0x10
        0x5b, 0x81, 0x0b, b'R', b'E', b'G', b'0',     // FieldOp PkgLength(11) "REG0"
        0x61,                                         //   FieldFlags: ByteAcc, NoLock, UpdateRule 3
        b'F', b'L', b'D', b'0', 0x04,                 //   NamedField "FLD0" PkgLength(4)
        0x14, 0x0c, b'M', b'A', b'I', b'N', 0x00,     // MethodOp PkgLength(12) "MAIN" flags 0
        0x70, 0x01, b'F', b'L', b'D', b'0',           //   StoreOp One "FLD0"
    ];
    let (s, f) = both(&aml, &["\\MAIN"]);
    assert_eq!(f.results, vec![Err("InvalidFieldFlags".to_string())]);
    assert!(stock_panicked(&s.results[0], "unreachable"), "{s:?}");
}

/// ```text
/// OperationRegion (REG0, SystemMemory, Zero, 0x10)
/// Field (REG0, AnyAcc, NoLock, Preserve) { FLD0, 0x0FFFFFFF }
/// Method (MAIN, 0) { Return (FLD0) }
/// ```
/// A field length is a PkgLength, so up to 2^28 bits; stock allocates for it before reading.
/// Fix 2 caps a buffer-typed field read at `MAX_OBJECT_BYTES` (a policy limit). Patched crate
/// only: stock would allocate 256 MiB and then make 2^28/8 handler reads.
#[test]
fn field_read_over_the_object_limit() {
    #[rustfmt::skip]
    let aml = [
        0x5b, 0x80, b'R', b'E', b'G', b'0',           // OpRegionOp "REG0"
        0x00, 0x00, 0x0a, 0x10,                       //   SystemMemory, Zero, BytePrefix 0x10
        0x5b, 0x81, 0x0e, b'R', b'E', b'G', b'0',     // FieldOp PkgLength(14) "REG0"
        0x00,                                         //   FieldFlags: AnyAcc, NoLock, Preserve
        b'F', b'L', b'D', b'0',                       //   NamedField "FLD0"
        0xcf, 0xff, 0xff, 0xff,                       //     PkgLength 0x0FFFFFFF (4-byte form)
        0x14, 0x0b, b'M', b'A', b'I', b'N', 0x00,     // MethodOp PkgLength(11) "MAIN" flags 0
        0xa4, b'F', b'L', b'D', b'0',                 //   ReturnOp "FLD0"
    ];
    let f = patched(&aml, &["\\MAIN"]);
    assert_eq!(f.load, Ok(()));
    assert_eq!(
        f.results,
        vec![Err(malformed("field longer than the object size limit"))]
    );
}

// ---- Stores and opcodes the crate does not implement -------------------------

/// ```text
/// Device (DEV0) {}
/// Method (MAIN, 0) { Notify (DEV0, One) }
/// Method (COPY, 0) { CopyObject (One, Local0)
///                    Return (Local0) }
/// Method (STOM, 0) { Store (One, MAIN) }   // a Method is not a store target
/// ```
/// ACPI 6.5 §19.6 `Notify`, `CopyObject`, `Store`. Stock panics on each
/// (`mod.rs:986` "Unexpected operation has created in-flight op!", `mod.rs:1632` `todo!()`,
/// `mod.rs:2339` "Stores to objects like ... are not yet supported"). Fix 2 fails closed with
/// `LibUnimplemented` and a `LOGANFY fail-closed:` log line: none of these returns a made-up
/// value.
#[test]
fn unimplemented_operations_fail_closed() {
    #[rustfmt::skip]
    let aml = [
        0x5b, 0x82, 0x05, b'D', b'E', b'V', b'0',     // DeviceOp PkgLength(5) "DEV0"
        0x14, 0x0c, b'M', b'A', b'I', b'N', 0x00,     // MethodOp PkgLength(12) "MAIN" flags 0
        0x86, b'D', b'E', b'V', b'0', 0x01,           //   NotifyOp "DEV0" One
        0x14, 0x0b, b'C', b'O', b'P', b'Y', 0x00,     // MethodOp PkgLength(11) "COPY" flags 0
        0x9d, 0x01, 0x60,                             //   CopyObjectOp One Local0
        0xa4, 0x60,                                   //   ReturnOp Local0
        0x14, 0x0c, b'S', b'T', b'O', b'M', 0x00,     // MethodOp PkgLength(12) "STOM" flags 0
        0x70, 0x01, b'M', b'A', b'I', b'N',           //   StoreOp One "MAIN"
    ];
    let (s, f) = both(&aml, &["\\MAIN", "\\COPY", "\\STOM"]);
    let u = Err("LibUnimplemented".to_string());
    assert_eq!(f.results, vec![u.clone(), u.clone(), u]);
    assert!(
        stock_panicked(&s.results[0], "Unexpected operation"),
        "{s:?}"
    );
    assert!(
        stock_panicked(&s.results[1], "not yet implemented"),
        "{s:?}"
    );
    assert!(stock_panicked(&s.results[2], "not yet supported"), "{s:?}");
}

// ---- Resource templates ------------------------------------------------------

/// Decode `bytes` as a resource template with both crates, catching a panic.
fn resources(bytes: &[u8]) -> (Result<String, String>, Result<String, String>) {
    acpi_aml_grade::recorder::install_hooks();
    let catch = |f: &dyn Fn() -> String| {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).map_err(|e| {
            e.downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default()
        })
    };
    let b = bytes.to_vec();
    let s = catch(&|| {
        let o = acpi_stock::aml::object::Object::Buffer(b.clone()).wrap();
        format!(
            "{:?}",
            acpi_stock::aml::resource::resource_descriptor_list(o).map(|r| r.len())
        )
    });
    let f = catch(&|| {
        let o = acpi_fix1::aml::object::Object::Buffer(b.clone()).wrap();
        format!(
            "{:?}",
            acpi_fix1::aml::resource::resource_descriptor_list(o).map(|r| r.len())
        )
    });
    (s, f)
}

/// A large descriptor cut off inside its header, a large descriptor shorter than its length
/// field, and a small one shorter than its length bits. ACPI 6.5 §6.4.2 (small descriptor:
/// byte 0 bits 0-2 are the length) and §6.4.3 (large: bytes 1-2 are the length). Stock indexes
/// and splits unchecked (`resource.rs:124` "mid > len" and the large-header reads).
#[test]
fn truncated_resource_descriptors() {
    #[rustfmt::skip]
    let cases: [&[u8]; 3] = [
        &[0x86, 0x09],                                // large type 0x06 (Fixed Memory32), header cut off
        &[0x86, 0x09, 0x00],                          // length 9, no data
        &[0x22],                                      // small type 0x04 (IRQ) length 2, no data
    ];
    for bytes in cases {
        let (s, f) = resources(bytes);
        assert_eq!(
            f,
            Ok("Err(InvalidResourceDescriptor)".to_string()),
            "{bytes:02x?}"
        );
        assert!(s.is_err(), "stock did not panic on {bytes:02x?}: {s:?}");
    }
}

// ---- Results built from operands (review #104, Jarvis 2e/4a and 3d) ----------

/// ```text
/// Method (MAIN, 0) { Store (Buffer (0x100000) {}, Local0)
///                    Return (SizeOf (Concat (Local0, Local0))) }
/// ```
/// Two operands each at `MAX_OBJECT_BYTES` (1 MiB). Stock returns the 2 MiB size; Fix 2 caps the
/// result of an operator that grows its operands (`check_object_size`). Input from the
/// cross-family review of #104 (item 2e).
#[test]
fn concat_result_over_the_object_limit() {
    #[rustfmt::skip]
    let aml = [
        0x14, 0x15, b'M', b'A', b'I', b'N', 0x00,     // MethodOp PkgLength(21) "MAIN" flags 0
        0x70, 0x11, 0x06, 0x0c, 0x00, 0x00, 0x10, 0x00, //   StoreOp BufferOp PkgLength(6) DWordPrefix 0x100000
        0x60,                                         //     Local0
        0xa4, 0x87, 0x73, 0x60, 0x60, 0x00,           //   ReturnOp SizeOfOp ConcatOp Local0 Local0 NullName
    ];
    let (s, f) = both(&aml, &["\\MAIN"]);
    assert_eq!(s.results, vec![Ok("Integer(2097152)".to_string())]);
    assert_eq!(
        f.results,
        vec![Err(malformed(
            "result larger than the interpreter's object limit"
        ))]
    );
}

/// ```text
/// Method (MAIN, 0) { Return (SizeOf (ToHexString (Buffer (0x40000) {}))) }
/// ```
/// `ToHexString` of a Buffer writes `0x00,` per byte: 256 KiB of input is a 1 310 719-byte String,
/// past the 1 MiB limit. Stock returns that size.
#[test]
fn to_hex_string_result_over_the_object_limit() {
    #[rustfmt::skip]
    let aml = [
        0x14, 0x11, b'M', b'A', b'I', b'N', 0x00,     // MethodOp PkgLength(17) "MAIN" flags 0
        0xa4, 0x87, 0x98,                             //   ReturnOp SizeOfOp ToHexStringOp
        0x11, 0x06, 0x0c, 0x00, 0x00, 0x04, 0x00,     //     BufferOp PkgLength(6) DWordPrefix 0x40000
        0x00,                                         //     NullName
    ];
    let (s, f) = both(&aml, &["\\MAIN"]);
    assert_eq!(s.results, vec![Ok("Integer(1310719)".to_string())]);
    assert_eq!(
        f.results,
        vec![Err(malformed(
            "result larger than the interpreter's object limit"
        ))]
    );
}

/// ```text
/// Method (MAIN, 0) { Return (Index (Buffer (1) { 0xAA }, One)) }
/// ```
/// An `Index` at or past the end is an error, not an empty object: ACPICA returns
/// AE_AML_BUFFER_LIMIT (and AE_AML_STRING_LIMIT / AE_AML_PACKAGE_LIMIT for the other source types),
/// `exoparg2.c` `AcpiExOpcode_2A_1T_1R`. The crate already returns `IndexOutOfBounds`; this pins
/// it. Input from the cross-family review of #104 (item 3d, which the dispatch had worded wrongly).
#[test]
fn index_past_the_end_is_an_error() {
    #[rustfmt::skip]
    let aml = [
        0x14, 0x0f, b'M', b'A', b'I', b'N', 0x00,     // MethodOp PkgLength(15) "MAIN" flags 0
        0xa4, 0x88,                                   //   ReturnOp IndexOp
        0x11, 0x04, 0x0a, 0x01, 0xaa,                 //     BufferOp PkgLength(4) BytePrefix 1 { 0xAA }
        0x01, 0x00,                                   //     One, NullName
    ];
    let (s, f) = both(&aml, &["\\MAIN"]);
    assert_eq!(f.results, vec![Err("IndexOutOfBounds".to_string())]);
    assert_eq!(s.results, f.results);
}
