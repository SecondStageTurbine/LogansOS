// SPDX-License-Identifier: MPL-2.0
//! Corpus loading: one directory per machine, one file per ACPI table, the
//! table identified by its header signature (never by file name, so the
//! Windows `acpidump -b` layout, the Linux `/sys/firmware/acpi/tables` layout
//! and the QEMU extractor layout all load the same way).

use crate::sha256::sha256_hex;
use std::fs;
use std::path::{Path, PathBuf};

/// Files in a set directory that are metadata, not tables.
const METADATA: &[&str] = &[
    "provenance.json",
    "SHA256SUMS",
    "README.md",
    "PROVENANCE.md",
    "bios.txt",
    "machine.txt",
    "acpidump-version.txt",
    "probes.json",
];

/// Tables that must never enter the corpus: MSDM carries the OEM Windows
/// licence key, SLIC the OEM licensing data. Matched by header signature.
pub const FORBIDDEN: &[&str] = &["MSDM", "SLIC"];

#[derive(Clone, Debug)]
pub struct Table {
    pub file: String,
    pub signature: String,
    pub bytes: Vec<u8>,
}

impl Table {
    /// The AML byte stream of a DSDT/SSDT: everything after the 36-byte SDT
    /// header, bounded by the header's own length field.
    pub fn aml_body(&self) -> &[u8] {
        let len = self.declared_len().min(self.bytes.len());
        &self.bytes[36.min(len)..len]
    }

    pub fn declared_len(&self) -> usize {
        if self.bytes.len() < 8 {
            return self.bytes.len();
        }
        u32::from_le_bytes(self.bytes[4..8].try_into().unwrap()) as usize
    }

    pub fn revision(&self) -> u8 {
        self.bytes.get(8).copied().unwrap_or(0)
    }
}

#[derive(Clone, Debug)]
pub struct CorpusSet {
    pub name: String,
    pub dir: PathBuf,
    pub tables: Vec<Table>,
    /// `Some(true)` when SHA256SUMS exists and every line verifies,
    /// `Some(false)` on any mismatch or missing file, `None` without the file.
    pub sums_ok: Option<bool>,
    pub sums_detail: Vec<String>,
    /// Files whose header signature is in `FORBIDDEN`. They are never loaded
    /// or graded; the gate test fails while any is present.
    pub forbidden: Vec<String>,
    /// Device probes from `probes.json`, if the set has one.
    pub probes: Vec<crate::probe::Probe>,
}

impl CorpusSet {
    pub fn dsdt(&self) -> Option<&Table> {
        self.tables.iter().find(|t| t.signature == "DSDT")
    }

    /// SSDTs in firmware order: the numeric suffix of the file name
    /// (`ssdt3.dat`, `SSDT3`), which both acpidump and Linux assign in
    /// XSDT order.
    pub fn ssdts(&self) -> Vec<&Table> {
        let mut v: Vec<&Table> = self
            .tables
            .iter()
            .filter(|t| t.signature == "SSDT")
            .collect();
        v.sort_by_key(|t| (numeric_suffix(&t.file), t.file.clone()));
        v
    }

    pub fn find(&self, sig: &str) -> Option<&Table> {
        self.tables.iter().find(|t| t.signature == sig)
    }
}

fn numeric_suffix(name: &str) -> u64 {
    let stem = name.split('.').next().unwrap_or(name);
    let digits: String = stem
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    digits.parse().unwrap_or(0)
}

fn signature_of(bytes: &[u8]) -> String {
    if bytes.len() >= 8 && &bytes[..8] == b"RSD PTR " {
        return "RSDP".to_string();
    }
    if bytes.len() < 4 {
        return "????".to_string();
    }
    bytes[..4]
        .iter()
        .map(|&b| if b.is_ascii_graphic() { b as char } else { '?' })
        .collect()
}

pub fn load_set(dir: &Path) -> std::io::Result<CorpusSet> {
    let name = dir.file_name().unwrap().to_string_lossy().into_owned();
    let mut files: Vec<_> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| !METADATA.contains(&n.as_str()))
        .collect();
    files.sort();
    let mut tables = Vec::new();
    let mut forbidden = Vec::new();
    for f in files {
        let bytes = fs::read(dir.join(&f))?;
        let signature = signature_of(&bytes);
        if FORBIDDEN.contains(&signature.as_str()) {
            forbidden.push(format!("{f} ({signature})"));
            continue;
        }
        tables.push(Table {
            signature,
            file: f,
            bytes,
        });
    }
    let (sums_ok, sums_detail) = verify_sums(dir, &tables);
    Ok(CorpusSet {
        name,
        dir: dir.to_path_buf(),
        tables,
        sums_ok,
        sums_detail,
        forbidden,
        probes: fs::read_to_string(dir.join("probes.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .map(|v| crate::probe::parse_probes(&v))
            .unwrap_or_default(),
    })
}

fn verify_sums(dir: &Path, tables: &[Table]) -> (Option<bool>, Vec<String>) {
    let Ok(text) = fs::read_to_string(dir.join("SHA256SUMS")) else {
        return (None, vec!["no SHA256SUMS".to_string()]);
    };
    let mut ok = true;
    let mut detail = Vec::new();
    let mut listed = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let Some((sum, file)) = line.split_once("  ") else {
            ok = false;
            detail.push(format!("malformed line: {line}"));
            continue;
        };
        listed.push(file.to_string());
        match tables.iter().find(|t| t.file == file) {
            Some(t) if sha256_hex(&t.bytes) == sum => {}
            Some(_) => {
                ok = false;
                detail.push(format!("{file}: checksum mismatch"));
            }
            None => {
                ok = false;
                detail.push(format!("{file}: listed but missing"));
            }
        }
    }
    for t in tables {
        if !listed.contains(&t.file) {
            ok = false;
            detail.push(format!("{}: present but not listed", t.file));
        }
    }
    (Some(ok), detail)
}

/// Every set directory under the corpus root, sorted by name.
pub fn load_corpus(root: &Path) -> std::io::Result<Vec<CorpusSet>> {
    let mut dirs: Vec<_> = fs::read_dir(root)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.path())
        .collect();
    dirs.sort();
    dirs.iter().map(|d| load_set(d)).collect()
}
