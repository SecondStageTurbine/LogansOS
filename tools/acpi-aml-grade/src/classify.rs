// SPDX-License-Identifier: MPL-2.0
//! The failure taxonomy. Every failure lands in exactly one class with a
//! named cause; `Unclassified` exists only so the gate can detect a failure
//! no rule explains (the corpus test fails on any).
//!
//! Classes (the rung brief's four, plus one extension):
//! - `ParseError`: the byte stream is not well-formed AML as the crate reads
//!   it. Neither stock crate reports a byte offset; the failure records that.
//! - `Unimplemented`: well-formed AML using an opcode or feature the crate
//!   does not implement, named (opcode + value, or the `todo!()` site).
//! - `TypeSemantics`: the crate executed the AML and reported (or panicked
//!   on) a type, namespace or semantics error.
//! - `OutOfRegionAccess`: a handler access outside every region the set's
//!   own tables declare (OpRegions + FADT fixed blocks), with the mechanical
//!   diagnosis of how the address was formed where one fits.
//! - `NonTermination` (extension): an evaluation spent the access budget.

use crate::recorder::Access;
use crate::report::{Mode, Phase, Region};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Class {
    ParseError,
    Unimplemented,
    TypeSemantics,
    OutOfRegionAccess,
    NonTermination,
    Unclassified,
}

impl Class {
    pub fn name(self) -> &'static str {
        match self {
            Class::ParseError => "parse-error",
            Class::Unimplemented => "unimplemented",
            Class::TypeSemantics => "type-semantics",
            Class::OutOfRegionAccess => "out-of-region-access",
            Class::NonTermination => "non-termination",
            Class::Unclassified => "UNCLASSIFIED",
        }
    }
}

/// ACPI 6.5 §20.3 extended opcodes (second byte after ExtOpPrefix 0x5B).
/// Cross-checked against `acpi` 6.1.1's decode table (src/aml/mod.rs).
pub const EXT_OPS: &[(u8, &str)] = &[
    (0x01, "Mutex"),
    (0x02, "Event"),
    (0x12, "CondRefOf"),
    (0x13, "CreateField"),
    (0x1f, "LoadTable"),
    (0x20, "Load"),
    (0x21, "Stall"),
    (0x22, "Sleep"),
    (0x23, "Acquire"),
    (0x24, "Signal"),
    (0x25, "Wait"),
    (0x26, "Reset"),
    (0x27, "Release"),
    (0x28, "FromBCD"),
    (0x29, "ToBCD"),
    (0x30, "Revision"),
    (0x31, "Debug"),
    (0x32, "Fatal"),
    (0x33, "Timer"),
    (0x80, "OpRegion"),
    (0x81, "Field"),
    (0x82, "Device"),
    (0x83, "Processor"),
    (0x84, "PowerRes"),
    (0x85, "ThermalZone"),
    (0x86, "IndexField"),
    (0x87, "BankField"),
    (0x88, "DataRegion"),
];

/// ACPI 6.5 §20.3 single-byte operator opcodes (data/name/local/arg
/// encodings omitted: they are never "unimplemented").
pub const OPS: &[(u8, &str)] = &[
    (0x06, "Alias"),
    (0x08, "Name"),
    (0x10, "Scope"),
    (0x11, "Buffer"),
    (0x12, "Package"),
    (0x13, "VarPackage"),
    (0x14, "Method"),
    (0x15, "External"),
    (0x70, "Store"),
    (0x71, "RefOf"),
    (0x72, "Add"),
    (0x73, "Concat"),
    (0x74, "Subtract"),
    (0x75, "Increment"),
    (0x76, "Decrement"),
    (0x77, "Multiply"),
    (0x78, "Divide"),
    (0x79, "ShiftLeft"),
    (0x7a, "ShiftRight"),
    (0x7b, "And"),
    (0x7c, "Nand"),
    (0x7d, "Or"),
    (0x7e, "Nor"),
    (0x7f, "Xor"),
    (0x80, "Not"),
    (0x81, "FindSetLeftBit"),
    (0x82, "FindSetRightBit"),
    (0x83, "DerefOf"),
    (0x84, "ConcatRes"),
    (0x85, "Mod"),
    (0x86, "Notify"),
    (0x87, "SizeOf"),
    (0x88, "Index"),
    (0x89, "Match"),
    (0x8a, "CreateDWordField"),
    (0x8b, "CreateWordField"),
    (0x8c, "CreateByteField"),
    (0x8d, "CreateBitField"),
    (0x8e, "ObjectType"),
    (0x8f, "CreateQWordField"),
    (0x90, "LAnd"),
    (0x91, "LOr"),
    (0x92, "LNot"),
    (0x93, "LEqual"),
    (0x94, "LGreater"),
    (0x95, "LLess"),
    (0x96, "ToBuffer"),
    (0x97, "ToDecimalString"),
    (0x98, "ToHexString"),
    (0x99, "ToInteger"),
    (0x9c, "ToString"),
    (0x9d, "CopyObject"),
    (0x9e, "Mid"),
    (0x9f, "Continue"),
    (0xa0, "If"),
    (0xa1, "Else"),
    (0xa2, "While"),
    (0xa3, "Noop"),
    (0xa4, "Return"),
    (0xa5, "Break"),
    (0xcc, "BreakPoint"),
];

/// Extended opcodes `aml` 0.16.4 has a parser arm for (every
/// `ext_opcode(opcode::EXT_*)` in its source).
const AML016_EXT: &[u8] = &[
    0x01, 0x12, 0x13, 0x30, 0x31, 0x32, 0x80, 0x81, 0x82, 0x83, 0x84, 0x85,
];

/// Single-byte operators `aml` 0.16.4 has a parser arm for (every
/// `opcode(opcode::DEF_*)` in its source).
const AML016_OPS: &[u8] = &[
    0x06, 0x08, 0x10, 0x11, 0x12, 0x14, 0x15, 0x70, 0x72, 0x73, 0x75, 0x76, 0x79, 0x7a, 0x7b, 0x84,
    0x8a, 0x8b, 0x8c, 0x8d, 0x8e, 0x8f, 0x90, 0x91, 0x92, 0x93, 0x94, 0x95, 0x99, 0x9e, 0x9f, 0xa0,
    0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xcc,
];

fn ext_name(b: u8) -> Option<&'static str> {
    EXT_OPS.iter().find(|(v, _)| *v == b).map(|(_, n)| *n)
}

fn op_name(b: u8) -> Option<&'static str> {
    OPS.iter().find(|(v, _)| *v == b).map(|(_, n)| *n)
}

/// Unsupported extended opcodes reachable from `target`: the method's own
/// body plus, when `follow` is set, every method whose NameSeg appears in a
/// body already reached. For a table load `target` is the table and its body
/// is scanned alone (the table already holds its own methods). Over-
/// approximate by design (a NameSeg match is not a proven call, and a 0x5B
/// byte inside a data buffer looks like a prefix), so the result is a
/// candidate set, reported as such.
fn unsupported_ext_in_call_path(
    target: &str,
    methods: &BTreeMap<String, Vec<u8>>,
    supported: &[u8],
    follow: bool,
) -> Vec<(u8, &'static str, String)> {
    let seg_of = |p: &str| p.rsplit(['.', '\\']).next().unwrap_or("").to_string();
    let mut todo = vec![target.to_string()];
    let mut seen = std::collections::BTreeSet::new();
    let mut found = BTreeMap::new();
    while let Some(m) = todo.pop() {
        if !seen.insert(m.clone()) {
            continue;
        }
        let Some(body) = methods.get(&m) else {
            continue;
        };
        for w in body.windows(2) {
            if w[0] == 0x5b
                && !supported.contains(&w[1])
                && let Some(n) = ext_name(w[1])
            {
                found.entry(w[1]).or_insert((n, m.clone()));
            }
        }
        if !follow {
            continue;
        }
        for (other, _) in methods.iter() {
            let seg = seg_of(other);
            if seg.len() == 4 && body.windows(4).any(|w| w == seg.as_bytes()) {
                todo.push(other.clone());
            }
        }
    }
    found.into_iter().map(|(b, (n, m))| (b, n, m)).collect()
}

/// Methods reachable from `target` (by the same NameSeg over-approximation as
/// `unsupported_ext_in_call_path`) whose own body declares an
/// OperationRegion (`OpRegionOp`, 0x5B 0x80). Such a region exists only
/// while its method runs, often at a base computed from firmware state, so
/// the judge, which sees the regions left in the namespace afterwards, cannot
/// check an access to it. Table bodies are followed but not counted: a
/// table's own OperationRegions are in the namespace.
fn method_local_regions_in_call_path(
    target: &str,
    methods: &BTreeMap<String, Vec<u8>>,
) -> Vec<String> {
    let seg_of = |p: &str| p.rsplit(['.', '\\']).next().unwrap_or("").to_string();
    let is_table = |p: &str| p.contains(" (");
    // NameSeg -> the methods ending in it, so each body is scanned once.
    let mut by_seg: BTreeMap<[u8; 4], Vec<&String>> = BTreeMap::new();
    for m in methods.keys() {
        if let Ok(seg) = <[u8; 4]>::try_from(seg_of(m).as_bytes()) {
            by_seg.entry(seg).or_default().push(m);
        }
    }
    let mut todo = vec![target.to_string()];
    let mut seen = std::collections::BTreeSet::new();
    let mut found = Vec::new();
    while let Some(m) = todo.pop() {
        if !seen.insert(m.clone()) {
            continue;
        }
        let Some(body) = methods.get(&m) else {
            continue;
        };
        if !is_table(&m) && body.windows(2).any(|w| w == [0x5b, 0x80]) {
            found.push(m.clone());
        }
        for w in body.windows(4) {
            if let Some(callees) = by_seg.get(w) {
                todo.extend(
                    callees
                        .iter()
                        .filter(|c| !seen.contains(c.as_str()))
                        .map(|c| c.to_string()),
                );
            }
        }
    }
    found.sort();
    found
}

/// How an out-of-region address may have been formed from a declared
/// region of the same space. Two hypotheses are tested mechanically:
/// the region base was dropped (access = field byte offset only), or a
/// field's BIT offset was added to the base as if it were bytes.
fn diagnose_access(a: &Access, regions: &[Region]) -> (String, String) {
    let space = a.space.name();
    let same: Vec<_> = regions
        .iter()
        .filter(|r| r.space == Some(a.space))
        .collect();
    for r in &same {
        if a.addr + a.width as u64 <= r.length {
            return (
                format!("{space} field access drops the OpRegion base"),
                format!(
                    "region base dropped: {:#x} is a field offset inside {} (base {:#x}, len {:#x}); the access should have gone to {:#x}",
                    a.addr,
                    r.path,
                    r.base,
                    r.length,
                    r.base + a.addr
                ),
            );
        }
    }
    for r in &same {
        if a.addr > r.base && (a.addr - r.base).is_multiple_of(8) {
            let byte = (a.addr - r.base) / 8;
            if byte + a.width as u64 <= r.length {
                return (
                    format!("{space} field bit offset added to the OpRegion base as bytes"),
                    format!(
                        "bit offset used as byte offset: {:#x} = {} base {:#x} + bit offset {:#x}; the access should have gone to {:#x}",
                        a.addr,
                        r.path,
                        r.base,
                        a.addr - r.base,
                        r.base + byte
                    ),
                );
            }
        }
    }
    (
        format!("{space} access outside every declared region (address formation unexplained)"),
        format!(
            "no declared {space} region covers {:#x} (width {}), and neither address-formation hypothesis fits",
            a.addr, a.width
        ),
    )
}

fn variant(raw: &str) -> &str {
    raw.split(['(', '{', ' ']).next().unwrap_or(raw)
}

fn parse_byte(raw: &str) -> Option<u8> {
    let inner = raw.split_once('(')?.1.split_once(')')?.0;
    inner.trim().parse().ok()
}

pub struct Ctx<'a> {
    pub krate: &'a str,
    pub phase: Phase,
    pub target: &'a str,
    pub mode: Mode,
    pub raw: &'a str,
    pub access: Option<&'a Access>,
    pub regions: &'a [Region],
    pub methods: &'a BTreeMap<String, Vec<u8>>,
    /// Warn/error log lines the crate emitted during the failing call.
    pub logs: &'a [String],
}

/// A classified failure: its class, the failure-MODE key it counts under in
/// the ranked gap list (stable across instances), and per-instance detail.
#[derive(Clone, Debug)]
pub struct Verdict {
    pub class: Class,
    pub key: String,
    pub detail: String,
}

fn v(class: Class, key: impl Into<String>, detail: impl Into<String>) -> Verdict {
    Verdict {
        class,
        key: key.into(),
        detail: detail.into(),
    }
}

fn unclassified(detail: impl Into<String>) -> Verdict {
    v(Class::Unclassified, "UNCLASSIFIED", detail)
}

pub fn classify(c: &Ctx) -> Verdict {
    match c.mode {
        Mode::Access => {
            let Some(a) = c.access else {
                return unclassified("access failure without an access");
            };
            let (mut key, mut detail) = diagnose_access(a, c.regions);
            // Neither formation hypothesis fits: name a method-local region in the
            // call path if there is one (a candidate set, like the opcode scan).
            if key.ends_with("(address formation unexplained)") {
                let local = method_local_regions_in_call_path(c.target, c.methods);
                if !local.is_empty() {
                    let space = a.space.name();
                    key = format!(
                        "{space} access through a method-local OperationRegion (runtime base, not checkable by the judge)"
                    );
                    detail = format!(
                        "no region in the namespace covers {:#x} (width {}); candidates that declare an OperationRegion in the call path: {}",
                        a.addr,
                        a.width,
                        if local.len() > 6 {
                            format!("{} and {} more", local[..6].join(", "), local.len() - 6)
                        } else {
                            local.join(", ")
                        }
                    );
                }
            }
            return v(Class::OutOfRegionAccess, key, detail);
        }
        Mode::Budget => {
            return v(
                Class::NonTermination,
                "evaluation does not terminate under the zero model",
                format!(
                    "{} handler accesses without returning",
                    crate::recorder::ACCESS_BUDGET
                ),
            );
        }
        Mode::Panic => {
            let site = c.raw.rsplit_once(" @ ").map(|(_, l)| l).unwrap_or("?");
            let msg = c.raw.split(" @ ").next().unwrap_or("");
            if msg.starts_with("not yet implemented") || msg.starts_with("not implemented") {
                // `unimplemented!("GPIO Connection Descriptor")` names the feature itself.
                let named = msg
                    .split_once(": ")
                    .map(|(_, f)| format!(" \"{f}\""))
                    .unwrap_or_default();
                return v(
                    Class::Unimplemented,
                    format!("todo!()/unimplemented!(){named} at {site} (panics)"),
                    msg,
                );
            }
            return v(Class::TypeSemantics, format!("panic at {site}"), msg);
        }
        Mode::Error => {}
    }
    let var = variant(c.raw);
    match c.krate {
        "aml" => classify_aml016(c, var),
        "acpi" | "acpi+fix1" => classify_acpi6(var, c.raw, c.logs),
        _ => unclassified("unknown crate"),
    }
}

fn classify_aml016(c: &Ctx, var: &str) -> Verdict {
    match var {
        "UnexpectedByte" => {
            let Some(b) = parse_byte(c.raw) else {
                return unclassified("UnexpectedByte without a byte");
            };
            if b == 0x5b {
                let cands = unsupported_ext_in_call_path(
                    c.target,
                    c.methods,
                    AML016_EXT,
                    c.phase != Phase::Load,
                );
                if cands.is_empty() {
                    return unclassified(
                        "ExtOpPrefix with no unsupported candidate in the call path",
                    );
                }
                let names: Vec<_> = cands
                    .iter()
                    .map(|(b, n, _)| format!("{n} (0x5B 0x{b:02X})"))
                    .collect();
                let wheres: Vec<_> = cands
                    .iter()
                    .map(|(_, n, m)| format!("{n} in {m}"))
                    .collect();
                return v(
                    Class::Unimplemented,
                    format!("no parser arm for extended opcode(s): {}", names.join(", ")),
                    format!("candidates in the call path: {}", wheres.join("; ")),
                );
            }
            match op_name(b) {
                Some(n) if !AML016_OPS.contains(&b) => v(
                    Class::Unimplemented,
                    format!("no parser arm for opcode {n} (0x{b:02X})"),
                    "",
                ),
                _ => v(
                    Class::ParseError,
                    format!("unexpected byte 0x{b:02X}"),
                    "byte offset not exposed by aml 0.16.4",
                ),
            }
        }
        "UnexpectedEndOfStream"
        | "MalformedStream"
        | "InvalidNameSeg"
        | "InvalidPkgLength"
        | "InvalidFieldFlags"
        | "UnterminatedStringConstant"
        | "InvalidStringConstant"
        | "InvalidRegionSpace"
        | "MalformedPackage"
        | "MalformedBuffer"
        | "WrongParser" => v(
            Class::ParseError,
            var,
            "byte offset not exposed by aml 0.16.4",
        ),
        "Unimplemented" => unclassified("AmlError::Unimplemented carries no feature name"),
        "FatalError" => v(Class::TypeSemantics, "firmware executed DefFatal", ""),
        "IncompatibleValueConversion"
        | "InvalidStatusObject"
        | "InvalidShiftLeft"
        | "InvalidShiftRight"
        | "FieldRegionIsNotOpRegion"
        | "FieldInvalidAddress"
        | "FieldInvalidAccessSize"
        | "TypeCannotBeCompared"
        | "TypeCannotBeSliced"
        | "TypeCannotBeWrittenToBufferField"
        | "BufferFieldIndexesOutOfBounds"
        | "InvalidArgAccess"
        | "InvalidLocalAccess"
        | "TooManyArgs"
        | "BreakInInvalidPosition"
        | "ContinueInInvalidPosition"
        | "NotExecutingControlMethod"
        | "ValueDoesNotExist"
        | "NameCollision"
        | "LevelDoesNotExist"
        | "TriedToRemoveRootNamespace"
        | "EmptyNamesAreInvalid"
        | "InvalidNormalizedName"
        | "RootHasNoParent"
        | "PrtInvalidAddress"
        | "PrtInvalidPin"
        | "PrtInvalidSource"
        | "PrtInvalidGsi"
        | "PrtNoEntry"
        | "ReservedResourceType"
        | "ResourceDescriptorTooShort"
        | "ResourceDescriptorTooLong"
        | "UnexpectedResourceType" => v(Class::TypeSemantics, var, c.raw),
        _ => unclassified(format!("no rule for aml error {var}")),
    }
}

fn classify_acpi6(var: &str, raw: &str, logs: &[String]) -> Verdict {
    match var {
        "IllegalOpcode" => {
            let Some(op) = raw
                .split_once('(')
                .and_then(|(_, r)| r.trim_end_matches(')').parse::<u16>().ok())
            else {
                return unclassified("IllegalOpcode without a value");
            };
            let name = if op >> 8 == 0x5b {
                ext_name(op as u8)
            } else {
                op_name(op as u8)
            };
            match name {
                Some(n) => v(
                    Class::Unimplemented,
                    format!("opcode {n} (0x{op:X}) rejected"),
                    "",
                ),
                None => v(
                    Class::ParseError,
                    format!("illegal opcode 0x{op:X}"),
                    "byte offset not exposed by acpi 6.1.1",
                ),
            }
        }
        "RunOutOfStream"
        | "InvalidFieldFlags"
        | "InvalidName"
        | "InvalidNameSeg"
        | "InvalidNormalizedName"
        | "RootHasNoParent"
        | "EmptyNamesAreInvalid"
        | "NoCurrentOp"
        | "ElseFoundWithoutCorrespondingIf"
        | "ContinueOutsideOfWhile"
        | "BreakOutsideOfWhile" => v(
            Class::ParseError,
            var,
            "byte offset not exposed by acpi 6.1.1",
        ),
        // Stock `acpi` never names the feature behind `LibUnimplemented`. Each Fix 1 fail-closed
        // site logs `LOGANFY fail-closed: <feature>` first; any other `LibUnimplemented` stays
        // unclassified.
        "LibUnimplemented" => match logs
            .iter()
            .rev()
            .find_map(|l| l.split_once("LOGANFY fail-closed: ").map(|(_, f)| f))
        {
            Some(feature) => v(
                Class::Unimplemented,
                format!("fails closed (LibUnimplemented): {feature}"),
                raw,
            ),
            None => unclassified("AmlError::LibUnimplemented carries no feature name"),
        },
        // Fix 2: a malformed-bytecode check that fired; the string names it.
        "MalformedAml" => v(
            Class::ParseError,
            format!(
                "malformed AML ({})",
                raw.split_once('"')
                    .and_then(|(_, r)| r.split_once('"'))
                    .map(|(m, _)| m)
                    .unwrap_or(raw)
            ),
            raw,
        ),
        "NoHandlerForRegionAccess" => v(
            Class::Unimplemented,
            format!("no access path for {raw}"),
            "",
        ),
        "LevelDoesNotExist"
        | "NameCollision"
        | "ObjectDoesNotExist"
        | "MethodArgCountIncorrect"
        | "InvalidOperationOnObject"
        | "IndexOutOfBounds"
        | "ObjectNotOfExpectedType"
        | "InvalidResourceDescriptor"
        | "UnexpectedResourceType"
        | "MutexAcquireTimeout"
        | "PrtInvalidAddress"
        | "PrtInvalidPin"
        | "PrtInvalidGsi"
        | "PrtInvalidSource"
        | "PrtNoEntry" => v(Class::TypeSemantics, var, raw),
        _ => unclassified(format!("no rule for acpi error {var}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recorder::Space;

    fn region(base: u64, length: u64) -> Region {
        Region {
            space: Some(Space::PciConfig),
            space_name: "PCI_Config".into(),
            base,
            length,
            path: "\\R".into(),
        }
    }

    fn access(addr: u64) -> Access {
        Access {
            space: Space::PciConfig,
            addr,
            pci: None,
            width: 1,
            write: false,
            value: 0,
            eval: 0,
        }
    }

    #[test]
    fn diagnoses_dropped_base() {
        let (k, d) = diagnose_access(&access(0x2), &[region(0x60, 0xc)]);
        assert_eq!(k, "PCI_Config field access drops the OpRegion base");
        assert!(d.starts_with("region base dropped"), "{d}");
        assert!(d.ends_with("0x62"), "{d}");
    }

    #[test]
    fn diagnoses_bit_offset_as_bytes() {
        let (k, d) = diagnose_access(&access(0x70), &[region(0x60, 0xc)]);
        assert_eq!(
            k,
            "PCI_Config field bit offset added to the OpRegion base as bytes"
        );
        assert!(d.starts_with("bit offset used as byte offset"), "{d}");
        assert!(d.ends_with("0x62"), "{d}");
    }

    #[test]
    fn unexplained_access_is_still_named() {
        let (_, d) = diagnose_access(&access(0x1000), &[region(0x60, 0xc)]);
        assert!(
            d.starts_with("no declared PCI_Config region covers 0x1000"),
            "{d}"
        );
    }

    #[test]
    fn ext_scan_follows_namesegs() {
        let mut m = BTreeMap::new();
        // \A._STA calls CSTA (NameSeg bytes), whose body holds Acquire (5B 23).
        m.insert("\\A._STA".to_string(), b"\xa4CSTA\x00".to_vec());
        m.insert("\\A.CSTA".to_string(), vec![0x5b, 0x23, 0x00]);
        let c = unsupported_ext_in_call_path("\\A._STA", &m, AML016_EXT, true);
        assert_eq!(c, vec![(0x23, "Acquire", "\\A.CSTA".to_string())]);
    }
}
