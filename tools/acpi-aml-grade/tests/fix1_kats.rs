// SPDX-License-Identifier: MPL-2.0
//! OT-86 Fix 1 known-answer tests: hand-laid AML byte programs, each run on
//! the stock `acpi` 6.1.1 and on the vendored copy with the Fix 1 patches
//! (`third_party/acpi-loganfy`), through the same recording handler the grade
//! uses (`run_kat`).
//!
//! These are byte oracles in the sense of CLAUDE.md: every program is laid
//! out by hand from the AML grammar (ACPI 6.5 chapter 20), independently of
//! both interpreters, with each byte commented. Opcode values were read from
//! the raw ACPICA header `source/include/amlcode.h` (acpica/acpica @
//! e85aa3be), never from a summarised source. The `Connection` program and
//! the GenericSerialBus descriptor are copied byte for byte from the corpus
//! (`fixtures/acpi/optiplex-7080-linux-1.37.0/DSDT`, SHA-256 75b5e2ac…).
//! Each program was also disassembled with ACPICA `iasl -d` as an
//! independent decode of the bytes; the expected ASL is in each doc comment.
//!
//! Every test pins both columns: the Fix 1 answer exactly, and the stock
//! failure that motivated the item, so a KAT cannot pass on a crate that
//! never had the bug.

use acpi_aml_grade::corpus::{Table, load_set};
use acpi_aml_grade::grade_acpi6::{fix1, stock};
use acpi_aml_grade::recorder::Space;
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
    both_rev(2, aml, calls)
}

fn both_rev(rev: u8, aml: &[u8], calls: &[&str]) -> (Kat, Kat) {
    let f = facp();
    (
        stock::run_kat(&f, rev, aml, calls),
        fix1::run_kat(&f, rev, aml, calls),
    )
}

fn ok(v: &str) -> Result<String, String> {
    Ok(v.to_string())
}

fn err_starts(r: &Result<String, String>, prefix: &str) -> bool {
    matches!(r, Err(e) if e.starts_with(prefix))
}

// ---- G1: Local/Arg references unwrapped at every operand site ------------

/// ```text
/// Method (MAIN, 0) { Local0 = 0x80
///                    If (Local0) { Return (9) }
///                    Return (Zero) }
/// ```
/// ACPI 6.5 §19.6 `If`: the predicate is the Local's value.
#[test]
fn g1_if_predicate_is_a_local() {
    #[rustfmt::skip]
    let aml = [
        0x14, 0x12, b'M', b'A', b'I', b'N', 0x00,     // MethodOp PkgLength(18) "MAIN" flags 0
        0x70, 0x0a, 0x80, 0x60,                       //   StoreOp BytePrefix 0x80 Local0
        0xa0, 0x05, 0x60,                             //   IfOp PkgLength(5) Local0
        0xa4, 0x0a, 0x09,                             //     ReturnOp BytePrefix 9
        0xa4, 0x00,                                   //   ReturnOp Zero
    ];
    let (s, f) = both(&aml, &["\\MAIN"]);
    assert_eq!(f.load, Ok(()));
    assert_eq!(f.results, vec![ok("Integer(9)")]);
    assert!(
        err_starts(
            &s.results[0],
            "ObjectNotOfExpectedType { expected: Integer, got: Integer }"
        ),
        "{s:?}"
    );
}

/// ```text
/// Method (MAIN, 0) { Local0 = 3
///                    Local1 = Zero
///                    While (Local0) { Local1++
///                                     Local0-- }
///                    Return (Local1) }
/// ```
/// ACPI 6.5 §19.6 `While`, `Increment`, `Decrement`: Locals as the predicate and as the
/// SuperName that is read, updated and written back.
#[test]
fn g1_while_increment_decrement_on_locals() {
    #[rustfmt::skip]
    let aml = [
        0x14, 0x16, b'M', b'A', b'I', b'N', 0x00,     // MethodOp PkgLength(22) "MAIN" flags 0
        0x70, 0x0a, 0x03, 0x60,                       //   StoreOp BytePrefix 3 Local0
        0x70, 0x00, 0x61,                             //   StoreOp Zero Local1
        0xa2, 0x06, 0x60,                             //   WhileOp PkgLength(6) Local0
        0x75, 0x61,                                   //     IncrementOp Local1
        0x76, 0x60,                                   //     DecrementOp Local0
        0xa4, 0x61,                                   //   ReturnOp Local1
    ];
    let (s, f) = both(&aml, &["\\MAIN"]);
    assert_eq!(f.results, vec![ok("Integer(3)")]);
    assert!(
        err_starts(&s.results[0], "ObjectNotOfExpectedType"),
        "{s:?}"
    );
}

/// ```text
/// Method (MAIN, 0) { Local0 = 10
///                    Sleep (Local0)
///                    Stall (Local0)
///                    Local1 = "ABCD"
///                    Return (Mid (Local1, One, 2)) }
/// ```
/// ACPI 6.5 §19.6 `Sleep`, `Stall`, `Mid`: Local operands of the other operator sites.
#[test]
fn g1_sleep_stall_mid_take_locals() {
    #[rustfmt::skip]
    let aml = [
        0x14, 0x1f, b'M', b'A', b'I', b'N', 0x00,     // MethodOp PkgLength(31) "MAIN" flags 0
        0x70, 0x0a, 0x0a, 0x60,                       //   StoreOp BytePrefix 10 Local0
        0x5b, 0x22, 0x60,                             //   SleepOp Local0
        0x5b, 0x21, 0x60,                             //   StallOp Local0
        0x70, 0x0d, b'A', b'B', b'C', b'D', 0x00, 0x61, //   StoreOp StringPrefix "ABCD" Local1
        0xa4, 0x9e, 0x61, 0x01, 0x0a, 0x02, 0x00,     //   ReturnOp MidOp Local1 One BytePrefix 2 NullName
    ];
    let (s, f) = both(&aml, &["\\MAIN"]);
    assert_eq!(f.results, vec![ok("String(\"BC\")")]);
    assert!(
        err_starts(&s.results[0], "ObjectNotOfExpectedType"),
        "{s:?}"
    );
}

/// ```text
/// Method (MAIN, 0) { Local0 = Buffer (3) { 1, 2, 3 }
///                    CreateByteField (Local0, One, BF01)
///                    Return (BF01) }
/// ```
/// ACPI 6.5 §19.6 `CreateByteField` over the buffer a Local holds (G1), and the buffer
/// field read as a TermArg (G3): an Integer, since 8 bits fit the integer width.
#[test]
fn g1_g3_create_field_over_a_local_buffer() {
    #[rustfmt::skip]
    let aml = [
        0x14, 0x1b, b'M', b'A', b'I', b'N', 0x00,     // MethodOp PkgLength(27) "MAIN" flags 0
        0x70,                                         //   StoreOp
        0x11, 0x06, 0x0a, 0x03, 0x01, 0x02, 0x03,     //     BufferOp PkgLength(6) BytePrefix 3 {1,2,3}
        0x60,                                         //     Local0
        0x8c, 0x60, 0x01, b'B', b'F', b'0', b'1',     //   CreateByteFieldOp Local0 One "BF01"
        0xa4, b'B', b'F', b'0', b'1',                 //   ReturnOp "BF01"
    ];
    let (s, f) = both(&aml, &["\\MAIN"]);
    assert_eq!(f.results, vec![ok("Integer(2)")]);
    assert_ne!(s.results, f.results, "stock already right? {s:?}");
}

// ---- G2: DerefOf on a reference -------------------------------------------

/// ```text
/// Method (MAIN, 0) { Return (DerefOf (Index (Package (2) { 5, 7 }, One))) }
/// ```
/// ACPI 6.5 §19.6 `DerefOf`, `Index`.
#[test]
fn g2_derefof_index_package() {
    #[rustfmt::skip]
    let aml = [
        0x14, 0x12, b'M', b'A', b'I', b'N', 0x00,     // MethodOp PkgLength(18) "MAIN" flags 0
        0xa4, 0x83, 0x88,                             //   ReturnOp DerefOfOp IndexOp
        0x12, 0x06, 0x02, 0x0a, 0x05, 0x0a, 0x07,     //     PackageOp PkgLength(6) 2 elements: 5, 7
        0x01, 0x00,                                   //     One NullName
    ];
    let (s, f) = both(&aml, &["\\MAIN"]);
    assert_eq!(f.results, vec![ok("Integer(7)")]);
    assert!(
        err_starts(
            &s.results[0],
            "ObjectNotOfExpectedType { expected: Reference, got: Integer }"
        ),
        "{s:?}"
    );
}

// ---- G3: buffer fields read as values ---------------------------------------

/// ```text
/// Method (MAIN, 0) { Return (DerefOf (Index (Buffer (2) { 0x11, 0x22 }, One))) }
/// ```
/// ACPI 6.5 §19.6 `Index` on a Buffer gives a reference to one byte; `DerefOf` gives its value.
#[test]
fn g3_derefof_index_buffer_is_the_byte() {
    #[rustfmt::skip]
    let aml = [
        0x14, 0x11, b'M', b'A', b'I', b'N', 0x00,     // MethodOp PkgLength(17) "MAIN" flags 0
        0xa4, 0x83, 0x88,                             //   ReturnOp DerefOfOp IndexOp
        0x11, 0x05, 0x0a, 0x02, 0x11, 0x22,           //     BufferOp PkgLength(5) BytePrefix 2 {0x11,0x22}
        0x01, 0x00,                                   //     One NullName
    ];
    let (s, f) = both(&aml, &["\\MAIN"]);
    assert_eq!(f.results, vec![ok("Integer(34)")]);
    assert!(
        err_starts(&s.results[0], "ObjectNotOfExpectedType"),
        "{s:?}"
    );
}

/// ```text
/// Name (BUF0, Buffer (2) { 0x11, 0x22 })
/// CreateWordField (BUF0, Zero, WF00)
/// Method (MAIN, 0) { Return (WF00) }
/// ```
/// A named buffer field as a TermArg is read, like a field unit: Integer 0x2211.
#[test]
fn g3_named_buffer_field_is_read_as_a_termarg() {
    #[rustfmt::skip]
    let aml = [
        0x08, b'B', b'U', b'F', b'0',                 // NameOp "BUF0"
        0x11, 0x05, 0x0a, 0x02, 0x11, 0x22,           //   BufferOp PkgLength(5) BytePrefix 2 {0x11,0x22}
        0x8b, b'B', b'U', b'F', b'0', 0x00,           // CreateWordFieldOp "BUF0" Zero
        b'W', b'F', b'0', b'0',                       //   "WF00"
        0x14, 0x0b, b'M', b'A', b'I', b'N', 0x00,     // MethodOp PkgLength(11) "MAIN" flags 0
        0xa4, b'W', b'F', b'0', b'0',                 //   ReturnOp "WF00"
    ];
    let (s, f) = both(&aml, &["\\MAIN"]);
    assert_eq!(f.load, Ok(()));
    assert_eq!(f.results, vec![ok("Integer(8721)")]);
    assert!(
        matches!(&s.results[0], Ok(v) if v.starts_with("BufferField")),
        "stock should hand back the unread field: {s:?}"
    );
}

// ---- (d) stores through references ------------------------------------------

/// ```text
/// Name (PKG0, Package (2) { One, 2 })
/// Method (MAIN, 0) { Index (PKG0, One) = "AB"
///                    Return (DerefOf (Index (PKG0, One))) }
/// ```
/// ACPI 6.5 §19.6 `Index`: a store to a package element replaces the element; there is
/// no conversion to the old element's type (ACPICA `AcpiExStoreObjectToIndex` agrees).
#[test]
fn d_store_to_index_of_package_replaces_the_element() {
    #[rustfmt::skip]
    let aml = [
        0x08, b'P', b'K', b'G', b'0',                 // NameOp "PKG0"
        0x12, 0x05, 0x02, 0x01, 0x0a, 0x02,           //   PackageOp PkgLength(5) 2 elements: One, 2
        0x14, 0x1b, b'M', b'A', b'I', b'N', 0x00,     // MethodOp PkgLength(27) "MAIN" flags 0
        0x70, 0x0d, b'A', b'B', 0x00,                 //   StoreOp StringPrefix "AB"
        0x88, b'P', b'K', b'G', b'0', 0x01, 0x00,     //     IndexOp "PKG0" One NullName
        0xa4, 0x83, 0x88, b'P', b'K', b'G', b'0', 0x01, 0x00, // ReturnOp DerefOfOp IndexOp "PKG0" One NullName
    ];
    let (s, f) = both(&aml, &["\\MAIN"]);
    assert_eq!(f.results, vec![ok("String(\"AB\")")]);
    assert!(
        err_starts(&s.results[0], "panic: not yet implemented"),
        "{s:?}"
    );
}

/// ```text
/// Name (BUF1, Buffer (2) { 0, 0 })
/// Method (MAIN, 0) { Index (BUF1, Zero) = 0x01AB
///                    Return (BUF1) }
/// ```
/// A store through `Index` of a Buffer writes the source's least-significant byte.
#[test]
fn d_store_to_index_of_buffer_writes_the_low_byte() {
    #[rustfmt::skip]
    let aml = [
        0x08, b'B', b'U', b'F', b'1',                 // NameOp "BUF1"
        0x11, 0x05, 0x0a, 0x02, 0x00, 0x00,           //   BufferOp PkgLength(5) BytePrefix 2 {0,0}
        0x14, 0x16, b'M', b'A', b'I', b'N', 0x00,     // MethodOp PkgLength(22) "MAIN" flags 0
        0x70, 0x0b, 0xab, 0x01,                       //   StoreOp WordPrefix 0x01AB
        0x88, b'B', b'U', b'F', b'1', 0x00, 0x00,     //     IndexOp "BUF1" Zero NullName
        0xa4, b'B', b'U', b'F', b'1',                 //   ReturnOp "BUF1"
    ];
    let (s, f) = both(&aml, &["\\MAIN"]);
    assert_eq!(f.results, vec![ok("Buffer(ab00)")]);
    assert!(
        err_starts(&s.results[0], "panic: not yet implemented"),
        "{s:?}"
    );
}

/// ```text
/// Name (INT0, One)
/// Method (MAIN, 0) { RefOf (INT0) = Buffer (2) { 0x34, 0x12 }
///                    Return (INT0) }
/// ```
/// ACPI 6.5 §19.6 `RefOf`, `Store`; §19.3.5: a store through `RefOf` is a store to the named
/// object, converted to its type (Buffer to Integer: the low bytes, little-endian).
#[test]
fn d_store_through_refof_converts_to_the_target_type() {
    #[rustfmt::skip]
    let aml = [
        0x08, b'I', b'N', b'T', b'0', 0x01,           // NameOp "INT0" One
        0x14, 0x17, b'M', b'A', b'I', b'N', 0x00,     // MethodOp PkgLength(23) "MAIN" flags 0
        0x70,                                         //   StoreOp
        0x11, 0x05, 0x0a, 0x02, 0x34, 0x12,           //     BufferOp PkgLength(5) BytePrefix 2 {0x34,0x12}
        0x71, b'I', b'N', b'T', b'0',                 //     RefOfOp "INT0"
        0xa4, b'I', b'N', b'T', b'0',                 //   ReturnOp "INT0"
    ];
    let (s, f) = both(&aml, &["\\MAIN"]);
    assert_eq!(f.results, vec![ok("Integer(4660)")]);
    assert!(
        err_starts(&s.results[0], "panic: not yet implemented"),
        "{s:?}"
    );
}

// ---- (d) unary operators: arity and semantics --------------------------------

/// ```text
/// Method (MAIN, 0) { Return (Not (0x0F)) }
/// Method (FSLB, 0) { Return (FindSetLeftBit (One)) }
/// Method (FSL8, 0) { Return (FindSetLeftBit (0x80)) }
/// Method (FSRB, 0) { Return (FindSetRightBit (0x80)) }
/// Method (NOTT, 0) { Not (0x0F, Local0)
///                    Return (Local0) }
/// ```
/// ACPI 6.5 §19.6 `Not` (bitwise one's complement), `FindSetLeftBit` / `FindSetRightBit`
/// (one-based bit position, 0 for no bit set); each is `Op Operand Target` (§20.2.5.4).
#[test]
fn d_unary_ops_take_a_target_and_compute_per_spec() {
    #[rustfmt::skip]
    let aml = [
        0x14, 0x0b, b'M', b'A', b'I', b'N', 0x00,     // MethodOp PkgLength(11) "MAIN" flags 0
        0xa4, 0x80, 0x0a, 0x0f, 0x00,                 //   ReturnOp NotOp BytePrefix 0x0F NullName
        0x14, 0x0a, b'F', b'S', b'L', b'B', 0x00,     // MethodOp PkgLength(10) "FSLB" flags 0
        0xa4, 0x81, 0x01, 0x00,                       //   ReturnOp FindSetLeftBitOp One NullName
        0x14, 0x0b, b'F', b'S', b'L', b'8', 0x00,     // MethodOp PkgLength(11) "FSL8" flags 0
        0xa4, 0x81, 0x0a, 0x80, 0x00,                 //   ReturnOp FindSetLeftBitOp BytePrefix 0x80 NullName
        0x14, 0x0b, b'F', b'S', b'R', b'B', 0x00,     // MethodOp PkgLength(11) "FSRB" flags 0
        0xa4, 0x82, 0x0a, 0x80, 0x00,                 //   ReturnOp FindSetRightBitOp BytePrefix 0x80 NullName
        0x14, 0x0c, b'N', b'O', b'T', b'T', 0x00,     // MethodOp PkgLength(12) "NOTT" flags 0
        0x80, 0x0a, 0x0f, 0x60,                       //   NotOp BytePrefix 0x0F Local0
        0xa4, 0x60,                                   //   ReturnOp Local0
    ];
    let calls = ["\\MAIN", "\\FSLB", "\\FSL8", "\\FSRB", "\\NOTT"];
    let (s, f) = both(&aml, &calls);
    assert_eq!(
        f.results,
        vec![
            ok("Integer(18446744073709551600)"),
            ok("Integer(1)"),
            ok("Integer(8)"),
            ok("Integer(8)"),
            ok("Integer(18446744073709551600)"),
        ]
    );
    for r in &s.results {
        assert!(err_starts(r, "panic: explicit panic"), "{s:?}");
    }
    // A revision-1 table has 32-bit integers.
    let (_, f1) = both_rev(1, &aml, &["\\MAIN"]);
    assert_eq!(f1.results, vec![ok("Integer(4294967280)")]);
}

// ---- (a) PCI_Config region base ----------------------------------------------

/// ```text
/// OperationRegion (PCFG, PCI_Config, 0x60, 0x0C)
/// Field (PCFG, ByteAcc, NoLock, Preserve) { PIRA, 8, PIRB, 8 }
/// Method (MAIN, 0) { Return (PIRB) }
/// ```
/// ACPI 6.5 §19.6 `OperationRegion`: PIRB is config-space byte 0x60 + 1.
#[test]
fn a_pci_config_field_access_adds_the_region_base() {
    #[rustfmt::skip]
    let aml = [
        0x5b, 0x80, b'P', b'C', b'F', b'G', 0x02,     // OpRegionOp "PCFG" PCI_Config
        0x0a, 0x60, 0x0a, 0x0c,                       //   BytePrefix 0x60 BytePrefix 0x0C
        0x5b, 0x81, 0x10, b'P', b'C', b'F', b'G', 0x01, // FieldOp PkgLength(16) "PCFG" ByteAcc
        b'P', b'I', b'R', b'A', 0x08,                 //   "PIRA" 8 bits
        b'P', b'I', b'R', b'B', 0x08,                 //   "PIRB" 8 bits
        0x14, 0x0b, b'M', b'A', b'I', b'N', 0x00,     // MethodOp PkgLength(11) "MAIN" flags 0
        0xa4, b'P', b'I', b'R', b'B',                 //   ReturnOp "PIRB"
    ];
    let (s, f) = both(&aml, &["\\MAIN"]);
    let pci = |k: &Kat| {
        k.accesses
            .iter()
            .filter(|a| a.space == Space::PciConfig)
            .map(|a| a.addr)
            .collect::<Vec<_>>()
    };
    assert_eq!(f.results, vec![ok("Integer(0)")]);
    assert_eq!(pci(&f), vec![0x61]);
    assert_eq!(pci(&s), vec![0x01], "stock drops the base");
}

// ---- (b) context_stack / MethodLocals unwind ----------------------------------

/// ```text
/// Method (INNR, 0) { Return (DerefOf (5)) }      // an error in a nested method
/// Method (OUTR, 0) { INNR ()
///                    Return (One) }
/// Method (GOOD, 0) { Return (0x42) }
/// ```
/// `\OUTR` fails inside `INNR`. On stock the caller context it pushed stays on the stack, so
/// `\GOOD`'s `Return` pops it and runs on in `OUTR`, returning One. Fixed: 0x42.
#[test]
fn b_error_exit_unwinds_the_context_stack() {
    #[rustfmt::skip]
    let aml = [
        0x14, 0x0a, b'I', b'N', b'N', b'R', 0x00,     // MethodOp PkgLength(10) "INNR" flags 0
        0xa4, 0x83, 0x0a, 0x05,                       //   ReturnOp DerefOfOp BytePrefix 5
        0x14, 0x0c, b'O', b'U', b'T', b'R', 0x00,     // MethodOp PkgLength(12) "OUTR" flags 0
        b'I', b'N', b'N', b'R',                       //   "INNR" (a call, no arguments)
        0xa4, 0x01,                                   //   ReturnOp One
        0x14, 0x09, b'G', b'O', b'O', b'D', 0x00,     // MethodOp PkgLength(9) "GOOD" flags 0
        0xa4, 0x0a, 0x42,                             //   ReturnOp BytePrefix 0x42
    ];
    let (s, f) = both(&aml, &["\\OUTR", "\\GOOD"]);
    assert!(
        err_starts(&f.results[0], "ObjectNotOfExpectedType"),
        "{f:?}"
    );
    assert_eq!(f.results[1], ok("Integer(66)"));
    assert_eq!(
        s.results[1],
        ok("Integer(1)"),
        "stock returns from the stale context"
    );
}

/// ```text
/// Device (DEV0) { Method (_ADR, 0) { Return (0x00030000) }
///                 OperationRegion (PCF2, PCI_Config, Zero, 4)
///                 Field (PCF2, WordAcc, NoLock, Preserve) { VID0, 16 } }
/// Method (INN2, 0) { Return (\DEV0.VID0) }
/// Method (OUT2, 0) { Local0 = INN2 ()
///                    Return (Local0 + 5) }
/// ```
/// Reading VID0 inside the nested `INN2` evaluates `\DEV0._ADR` re-entrantly. Its `Return`
/// must not pop `OUT2`'s pushed context. Fixed: Integer(5), VID0 read from device 3
/// function 0. Stock: `_ADR`'s value lands in `OUT2`, whose `Return` then becomes `_ADR`'s
/// result (0x30005, so function 5 is read) and `OUT2` itself never completes.
#[test]
fn b_reentrant_evaluation_returns_to_its_own_caller() {
    #[rustfmt::skip]
    let aml = [
        0x5b, 0x82, 0x29, b'D', b'E', b'V', b'0',     // DeviceOp PkgLength(41) "DEV0"
        0x14, 0x0c, b'_', b'A', b'D', b'R', 0x00,     //   MethodOp PkgLength(12) "_ADR" flags 0
        0xa4, 0x0c, 0x00, 0x00, 0x03, 0x00,           //     ReturnOp DWordPrefix 0x00030000
        0x5b, 0x80, b'P', b'C', b'F', b'2', 0x02,     //   OpRegionOp "PCF2" PCI_Config
        0x00, 0x0a, 0x04,                             //     Zero BytePrefix 4
        0x5b, 0x81, 0x0b, b'P', b'C', b'F', b'2', 0x02, //   FieldOp PkgLength(11) "PCF2" WordAcc
        b'V', b'I', b'D', b'0', 0x10,                 //     "VID0" 16 bits
        0x14, 0x11, b'I', b'N', b'N', b'2', 0x00,     // MethodOp PkgLength(17) "INN2" flags 0
        0xa4, 0x5c, 0x2e,                             //   ReturnOp RootChar DualNamePrefix
        b'D', b'E', b'V', b'0', b'V', b'I', b'D', b'0', //   "DEV0" "VID0"
        0x14, 0x12, b'O', b'U', b'T', b'2', 0x00,     // MethodOp PkgLength(18) "OUT2" flags 0
        0x70, b'I', b'N', b'N', b'2', 0x60,           //   StoreOp "INN2" (a call) Local0
        0xa4, 0x72, 0x60, 0x0a, 0x05, 0x00,           //   ReturnOp AddOp Local0 BytePrefix 5 NullName
    ];
    let (s, f) = both(&aml, &["\\OUT2"]);
    assert_eq!(f.load, Ok(()));
    let pci = |k: &Kat| {
        k.accesses
            .iter()
            .filter(|a| a.space == Space::PciConfig)
            .map(|a| (a.pci, a.addr))
            .collect::<Vec<_>>()
    };
    assert_eq!(f.results, vec![ok("Integer(5)")]);
    assert_eq!(pci(&f), vec![(Some((0, 0, 3, 0)), 0)]);
    assert_eq!(s.results, vec![ok("Integer(0)")], "{s:?}");
    assert_eq!(pci(&s), vec![(Some((0, 0, 3, 5)), 0)], "{s:?}");
}

// ---- (c) parse-only Connection and extended AccessAs ---------------------------

/// The OptiPlex 7080 DSDT's GenericSerialBus field, byte for byte (DSDT offset 0x4ab9c),
/// plus a second field using the BufferData form of `Connection`:
/// ```text
/// OperationRegion (RCH1, GenericSerialBus, 0x1000, 0x0100)
/// Name (RES1, Buffer (2) { 0x79, 0x00 })
/// Field (RCH1, BufferAcc, NoLock, Preserve) { Connection (RES1), Offset (0x56),
///                                             AccessAs (BufferAcc, AttribBytes (0x21)), CMD0, 16 }
/// Field (RCH1, BufferAcc, NoLock, Preserve) { Connection (ResourceTemplate () {}), CMD1, 16 }
/// Name (AFTR, 0x77)
/// Method (RD00, 0) { Return (CMD0) }
/// Method (RD01, 0) { Return (CMD1) }
/// ```
/// ACPI 6.5 §20.2.5.2: `ConnectField := 0x02 NameString | 0x02 BufferData`;
/// `ExtendedAccessField := 0x03 AccessType ExtendedAccessAttrib AccessLength`. The table
/// loads past them (AFTR exists); the fields fail closed and never yield a value.
#[test]
fn c_connection_fields_parse_and_fail_closed() {
    #[rustfmt::skip]
    let aml = [
        0x5b, 0x80, b'R', b'C', b'H', b'1', 0x09,     // OpRegionOp "RCH1" GenericSerialBus
        0x0b, 0x00, 0x10, 0x0b, 0x00, 0x01,           //   WordPrefix 0x1000 WordPrefix 0x0100
        0x08, b'R', b'E', b'S', b'1',                 // NameOp "RES1"
        0x11, 0x05, 0x0a, 0x02, 0x79, 0x00,           //   BufferOp PkgLength(5) BytePrefix 2 {EndTag}
        // From the OptiPlex DSDT, unchanged:
        0x5b, 0x81, 0x17, b'R', b'C', b'H', b'1', 0x05, // FieldOp PkgLength(23) "RCH1" BufferAcc
        0x02, b'R', b'E', b'S', b'1',                 //   ConnectField NameString "RES1"
        0x00, 0x40, 0x2b,                             //   ReservedField PkgLength(0x2b0 bits = Offset 0x56)
        0x03, 0x05, 0x0b, 0x21,                       //   ExtendedAccessField BufferAcc AttribBytes 0x21
        b'C', b'M', b'D', b'0', 0x10,                 //   "CMD0" 16 bits
        // The BufferData form:
        0x5b, 0x81, 0x12, b'R', b'C', b'H', b'1', 0x05, // FieldOp PkgLength(18) "RCH1" BufferAcc
        0x02, 0x11, 0x05, 0x0a, 0x02, 0x79, 0x00,     //   ConnectField BufferOp PkgLength(5) 2 {EndTag}
        b'C', b'M', b'D', b'1', 0x10,                 //   "CMD1" 16 bits
        0x08, b'A', b'F', b'T', b'R', 0x0a, 0x77,     // NameOp "AFTR" BytePrefix 0x77
        0x14, 0x0b, b'R', b'D', b'0', b'0', 0x00,     // MethodOp PkgLength(11) "RD00" flags 0
        0xa4, b'C', b'M', b'D', b'0',                 //   ReturnOp "CMD0"
        0x14, 0x0b, b'R', b'D', b'0', b'1', 0x00,     // MethodOp PkgLength(11) "RD01" flags 0
        0xa4, b'C', b'M', b'D', b'1',                 //   ReturnOp "CMD1"
    ];
    let (s, f) = both(&aml, &["\\AFTR", "\\RD00", "\\RD01"]);
    assert_eq!(f.load, Ok(()));
    assert_eq!(f.results[0], ok("Integer(119)"));
    assert_eq!(f.results[1], Err("LibUnimplemented".to_string()));
    assert_eq!(f.results[2], Err("LibUnimplemented".to_string()));
    assert!(
        f.accesses.is_empty(),
        "a Connection field must not touch any region: {f:?}"
    );
    assert!(
        matches!(&s.load, Err(e) if e.starts_with("panic: not yet implemented: Connect field")),
        "{s:?}"
    );
}

// ---- (d) fail closed instead of panicking ---------------------------------------

/// ```text
/// OperationRegion (SIO0, SystemIO, 0x2E, 2)
/// Field (SIO0, ByteAcc, NoLock, Preserve) { INDX, 8, DATA, 8 }
/// IndexField (INDX, DATA, ByteAcc, NoLock, Preserve) { CR07, 8 }
/// BankField (SIO0, INDX, One, ByteAcc, NoLock, Preserve) { BF00, 8 }
/// Method (RDIX, 0) { Return (CR07) }
/// Method (WRIX, 0) { CR07 = One }
/// Method (RDBK, 0) { Return (BF00) }
/// ```
/// Fix 1 failed IndexField and BankField accesses closed; Fix 3 (L4) implements them
/// (`fix3_kats.rs`). Index writes go to INDX (0x2E) before each DATA (0x2F) access; the bank
/// value goes to INDX before the bank field's region access. The recording handler returns the
/// last value written to a port, so `RDBK` reads back the bank value 1 at 0x2E.
#[test]
fn d_index_and_bank_fields() {
    #[rustfmt::skip]
    let aml = [
        0x5b, 0x80, b'S', b'I', b'O', b'0', 0x01,     // OpRegionOp "SIO0" SystemIO
        0x0a, 0x2e, 0x0a, 0x02,                       //   BytePrefix 0x2E BytePrefix 2
        0x5b, 0x81, 0x10, b'S', b'I', b'O', b'0', 0x01, // FieldOp PkgLength(16) "SIO0" ByteAcc
        b'I', b'N', b'D', b'X', 0x08,                 //   "INDX" 8 bits
        b'D', b'A', b'T', b'A', 0x08,                 //   "DATA" 8 bits
        0x5b, 0x86, 0x0f, b'I', b'N', b'D', b'X',     // IndexFieldOp PkgLength(15) "INDX"
        b'D', b'A', b'T', b'A', 0x01,                 //   "DATA" ByteAcc
        b'C', b'R', b'0', b'7', 0x08,                 //   "CR07" 8 bits
        0x5b, 0x87, 0x10, b'S', b'I', b'O', b'0',     // BankFieldOp PkgLength(16) "SIO0"
        b'I', b'N', b'D', b'X', 0x01, 0x01,           //   "INDX" BankValue One ByteAcc
        b'B', b'F', b'0', b'0', 0x08,                 //   "BF00" 8 bits
        0x14, 0x0b, b'R', b'D', b'I', b'X', 0x00,     // MethodOp PkgLength(11) "RDIX" flags 0
        0xa4, b'C', b'R', b'0', b'7',                 //   ReturnOp "CR07"
        0x14, 0x0c, b'W', b'R', b'I', b'X', 0x00,     // MethodOp PkgLength(12) "WRIX" flags 0
        0x70, 0x01, b'C', b'R', b'0', b'7',           //   StoreOp One "CR07"
        0x14, 0x0b, b'R', b'D', b'B', b'K', 0x00,     // MethodOp PkgLength(11) "RDBK" flags 0
        0xa4, b'B', b'F', b'0', b'0',                 //   ReturnOp "BF00"
    ];
    let (s, f) = both(&aml, &["\\RDIX", "\\WRIX", "\\RDBK"]);
    assert_eq!(f.load, Ok(()));
    assert_eq!(f.results[0], ok("Integer(0)"));
    assert_eq!(f.results[2], ok("Integer(1)"));
    let io: Vec<_> = f
        .accesses
        .iter()
        .map(|a| (a.write, a.addr, a.value))
        .collect();
    assert_eq!(
        io,
        vec![
            (true, 0x2e, 0),
            (false, 0x2f, 0),
            (true, 0x2e, 0),
            (true, 0x2f, 1),
            (true, 0x2e, 1),
            (false, 0x2e, 1),
        ]
    );
    for r in &s.results {
        assert!(err_starts(r, "panic: not yet implemented"), "{s:?}");
    }
}

/// ```text
/// Method (BC05, 0) { Return (LEqual (Buffer (5) { 1, 2, 3, 4, 5 }, Buffer (5) { 1, 2, 3, 4, 5 })) }
/// Method (BC02, 0) { Return (LEqual (Buffer (2) { 1, 2 }, Buffer (2) { 1, 2 })) }
/// ```
/// Fix 1 made a comparison of buffers longer than 4 bytes fail closed; Fix 3 (L4) implements
/// it (`fix3_kats.rs`), so the 5-byte comparison is now true (Ones). The short-buffer result is
/// unchanged.
#[test]
fn d_long_buffer_comparison() {
    #[rustfmt::skip]
    let aml = [
        0x14, 0x1a, b'B', b'C', b'0', b'5', 0x00,     // MethodOp PkgLength(26) "BC05" flags 0
        0xa4, 0x93,                                   //   ReturnOp LEqualOp
        0x11, 0x08, 0x0a, 0x05, 0x01, 0x02, 0x03, 0x04, 0x05, // BufferOp PkgLength(8) 5 {1,2,3,4,5}
        0x11, 0x08, 0x0a, 0x05, 0x01, 0x02, 0x03, 0x04, 0x05, // BufferOp PkgLength(8) 5 {1,2,3,4,5}
        0x14, 0x14, b'B', b'C', b'0', b'2', 0x00,     // MethodOp PkgLength(20) "BC02" flags 0
        0xa4, 0x93,                                   //   ReturnOp LEqualOp
        0x11, 0x05, 0x0a, 0x02, 0x01, 0x02,           //     BufferOp PkgLength(5) 2 {1,2}
        0x11, 0x05, 0x0a, 0x02, 0x01, 0x02,           //     BufferOp PkgLength(5) 2 {1,2}
    ];
    let (s, f) = both(&aml, &["\\BC05", "\\BC02"]);
    assert_eq!(f.results[0], ok("Integer(18446744073709551615)"));
    assert_eq!(f.results[1], ok("Integer(18446744073709551615)"));
    assert!(
        err_starts(&s.results[0], "panic: range end index 5"),
        "{s:?}"
    );
    assert_eq!(s.results[1], f.results[1]);
}

/// The OptiPlex DSDT's `RES1` buffer contents, byte for byte (DSDT offset 0x4ab27): one
/// I2cSerialBusV2 descriptor, a GenericSerialBus Connection Descriptor (large item 0x0E,
/// so its first byte is 0x8E; ACPICA `ACPI_RESOURCE_NAME_SERIAL_BUS`), then the End Tag
/// (0x79 0x00). Fix 1 failed it closed; Fix 3 (L4) decodes it (`fix3_kats.rs`).
#[test]
fn d_generic_serial_bus_descriptor() {
    #[rustfmt::skip]
    let crs = [
        0x8e, 0x1e, 0x00,                             // Large item 0x0E, length 0x001E
        0x01, 0x00, 0x01, 0x02, 0x00, 0x00, 0x01,     //   revision, source index, type I2C, flags, type flags, type rev
        0x06, 0x00,                                   //   type data length
        0x80, 0x1a, 0x06, 0x00,                       //   connection speed 400000
        0x08, 0x00,                                   //   slave address 0x08
        b'\\', b'_', b'S', b'B', b'.', b'P', b'C', b'I', b'0', b'.', b'I', b'2', b'C', b'0', 0x00, // "\_SB.PCI0.I2C0"
        0x79, 0x00,                                   // End Tag
    ];
    assert_eq!(
        fix1::decode_resources_kat(&crs),
        Ok(r#"[I2cSerialBus(I2cSerialBusDescriptor { revision_id: 1, resource_source_index: 0, device_initiated: false, is_consumer: true, is_shared: false, ten_bit_addressing: false, type_revision_id: 1, connection_speed: 400000, slave_address: 8, vendor_data: [], resource_source: "\\_SB.PCI0.I2C0" })]"#.to_string())
    );
    assert!(
        matches!(stock::decode_resources_kat(&crs), Err(e) if e.starts_with("panic: not implemented: GenericSerialBus")),
        "stock should panic"
    );
}
