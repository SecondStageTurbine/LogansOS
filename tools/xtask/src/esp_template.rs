// SPDX-License-Identifier: MPL-2.0
//! OT-87 R4.2: the installer's ESP templates
//! (`docs/THINK-2026-10-27-ot87-installer.md` §4.3, §7.1).
//!
//! One FAT32 image per logical sector size (512 and 4096), the size of the
//! installed disk's ESP (`gpt_logic::LAYOUT_ESP_MIB`), formatted on the host
//! with the vendored `fatfs` (`third_party/fatfs`). Each holds Limine's
//! `\EFI\BOOT\BOOTX64.EFI`, `\limine.conf`, and the preallocated slot files
//! `\slot\A\…` and `\slot\B\…` at their maximum sizes, zero-filled.
//!
//! The slot map is not taken from `fatfs`: [`read_back`] walks the image it
//! wrote with this module's own FAT32 reader, finds each slot file's short
//! directory entry, follows its chain in both FATs, and refuses a chain that
//! is not contiguous. The kernel then writes only inside those extents and the
//! entries' `FileSize` fields (§7.1); it never formats or rewrites FAT.

use std::io::{Cursor, Write};

use crate::install_bundle_logic::{SlotEntry, SlotMap};

/// The ESP's size: the GPT layout's entry 1.
pub const ESP_BYTES: u64 = crate::gpt_logic::LAYOUT_ESP_MIB * 1024 * 1024;
/// One cluster is 4 KiB at both sector sizes (8 sectors at 512, one at 4096).
pub const CLUSTER_BYTES: u32 = 4096;
/// The volume serial, as R3's one-disk ESP ("LOGA").
pub const VOLUME_ID: u32 = 0x4c4f_4741;
pub const VOLUME_LABEL: [u8; 11] = *b"LOGANSOS   ";

/// `\limine.conf` is a fixed-size file: its text, then newlines to the end.
/// Rewriting it never changes its size or its chain.
pub const LIMINE_CONF_BYTES: u64 = 64 * 1024;

/// The files of one deployment slot and their maximum sizes (§7.2: a
/// deployment measured 127.5 MiB; these leave each file room to roughly
/// double). Two slots take 496 MiB of the 512 MiB ESP.
pub const SLOT_FILES: [(&str, u64); 5] = [
    ("kernel", 72 << 20),
    ("neo-init", 44 << 20),
    ("neo-worker", 44 << 20),
    ("neo-shell", 44 << 20),
    ("neo-arthur", 44 << 20),
];
pub const SLOTS: [&str; 2] = ["A", "B"];

/// The `limine.conf` an installed disk boots with: slot A. The installer's W5
/// writes this file's extent again (§4.7); slot B's entry is added when
/// OT-51 slice 1 ships (§7.1).
pub fn installed_limine_conf() -> String {
    let mut text = String::from(
        "# LogansOS, installed. Written by the installer (OT-87).\n\
         timeout: 0\n\
         default_entry: 1\n\
         interface_branding: LogansOS\n\
         interface_branding_colour: 6\n\
         serial: yes\n\
         \n\
         /LogansOS (slot A)\n\
         \x20   protocol: limine\n\
         \x20   path: boot():/slot/A/kernel\n\
         \x20   kaslr: yes\n",
    );
    for (name, _) in &SLOT_FILES[1..] {
        text.push_str(&format!("    module_path: boot():/slot/A/{name}\n"));
    }
    text
}

/// `text` padded with newlines to [`LIMINE_CONF_BYTES`].
pub fn padded_limine_conf(text: &str) -> Result<Vec<u8>, String> {
    let mut bytes = text.as_bytes().to_vec();
    if bytes.len() as u64 > LIMINE_CONF_BYTES {
        return Err(format!(
            "limine.conf is {} bytes, more than its {LIMINE_CONF_BYTES}-byte file",
            bytes.len()
        ));
    }
    bytes.resize(LIMINE_CONF_BYTES as usize, b'\n');
    Ok(bytes)
}

/// Every slot-mapped path, in slot-map order: `limine.conf`, then each slot's
/// files. `(path in the ESP, size)`.
pub fn slot_paths() -> Vec<(String, u64)> {
    let mut out = vec![("limine.conf".to_string(), LIMINE_CONF_BYTES)];
    for slot in SLOTS {
        for (name, size) in SLOT_FILES {
            out.push((format!("slot/{slot}/{name}"), size));
        }
    }
    out
}

/// Build the template for `sector_size`: the image and its slot map.
pub fn build(sector_size: u32, bootx64: &[u8]) -> Result<(Vec<u8>, SlotMap), String> {
    if sector_size != 512 && sector_size != 4096 {
        return Err(format!("unsupported sector size {sector_size}"));
    }
    let mut image = Cursor::new(vec![0u8; ESP_BYTES as usize]);
    let options = fatfs::FormatVolumeOptions::new()
        .bytes_per_sector(sector_size as u16)
        .bytes_per_cluster(CLUSTER_BYTES)
        .fat_type(fatfs::FatType::Fat32)
        .total_sectors((ESP_BYTES / sector_size as u64) as u32)
        .volume_id(VOLUME_ID)
        .volume_label(VOLUME_LABEL);
    fatfs::format_volume(&mut fatfs::StdIoWrapper::new(&mut image), options)
        .map_err(|e| format!("format the ESP: {e:?}"))?;
    {
        let options = fatfs::FsOptions::new().time_provider(FixedTime);
        let fs = fatfs::FileSystem::new(&mut image, options)
            .map_err(|e| format!("open the new ESP: {e:?}"))?;
        populate(&fs.root_dir(), bootx64)?;
        fs.unmount()
            .map_err(|e| format!("unmount the ESP: {e:?}"))?;
    }
    image.set_position(0);
    let image = image.into_inner();
    let map = read_back(&image)?;
    Ok((image, map))
}

/// Every entry's timestamp: 1980-01-01 00:00, the earliest valid FAT date,
/// so a template build is reproducible. (fatfs's own provider without
/// `chrono` writes raw zeros, which is month 0.)
#[derive(Debug, Clone, Copy)]
struct FixedTime;

impl fatfs::TimeProvider for FixedTime {
    fn get_current_date(&self) -> fatfs::Date {
        fatfs::Date::new(1980, 1, 1)
    }

    fn get_current_date_time(&self) -> fatfs::DateTime {
        fatfs::DateTime::new(fatfs::Date::new(1980, 1, 1), fatfs::Time::new(0, 0, 0, 0))
    }
}

/// Write the template's tree into `root`.
fn populate<IO: fatfs::ReadWriteSeek, TP: fatfs::TimeProvider, OCC: fatfs::OemCpConverter>(
    root: &fatfs::Dir<'_, IO, TP, OCC>,
    bootx64: &[u8],
) -> Result<(), String>
where
    IO::Error: std::fmt::Debug + 'static,
    std::io::Error: From<fatfs::Error<IO::Error>>,
{
    let io = |what: &str, e: &dyn std::fmt::Debug| format!("{what}: {e:?}");
    // Directories and small files first, so the slot files' clusters
    // follow one another.
    let boot = root
        .create_dir("EFI")
        .and_then(|d| d.create_dir("BOOT"))
        .map_err(|e| io("create EFI/BOOT", &e))?;
    let slot = root.create_dir("slot").map_err(|e| io("create slot", &e))?;
    for name in SLOTS {
        slot.create_dir(name)
            .map_err(|e| io(&format!("create slot/{name}"), &e))?;
    }
    let conf = padded_limine_conf(&installed_limine_conf())?;
    for (dir, name, bytes) in [
        (&boot, "BOOTX64.EFI", bootx64),
        (root, "limine.conf", &conf[..]),
    ] {
        dir.create_file(name)
            .map_err(|e| io(&format!("create {name}"), &e))?
            .write_all(bytes)
            .map_err(|e| io(&format!("write {name}"), &e))?;
    }
    let zeros = vec![0u8; 1 << 20];
    for (path, size) in slot_paths().into_iter().skip(1) {
        let mut file = root
            .create_file(&path)
            .map_err(|e| io(&format!("create {path}"), &e))?;
        let mut left = size;
        while left > 0 {
            let take = left.min(zeros.len() as u64) as usize;
            file.write_all(&zeros[..take])
                .map_err(|e| io(&format!("fill {path}"), &e))?;
            left -= take as u64;
        }
    }
    Ok(())
}

/// The FAT32 geometry of an image, from its boot sector.
#[derive(Clone, Copy, Debug)]
pub struct Geometry {
    pub sector: u64,
    pub sectors_per_cluster: u64,
    pub fat_start: u64,
    pub fat_sectors: u64,
    pub fats: u64,
    pub data_start: u64,
    pub root_cluster: u32,
    pub clusters: u32,
}

fn le16(b: &[u8], at: usize) -> u64 {
    u16::from_le_bytes([b[at], b[at + 1]]) as u64
}

fn le32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

/// The boot sector's BPB (FAT spec "Microsoft Extensible Firmware Initiative
/// FAT32 File System Specification" 1.03, §3.1 and §3.3).
pub fn geometry(image: &[u8]) -> Result<Geometry, String> {
    if image.len() < 512 || image[510] != 0x55 || image[511] != 0xaa {
        return Err("no boot sector signature".into());
    }
    let sector = le16(image, 11);
    let sectors_per_cluster = image[13] as u64;
    let reserved = le16(image, 14);
    let fats = image[16] as u64;
    let root_entries = le16(image, 17);
    let total16 = le16(image, 19);
    let fat16 = le16(image, 22);
    let total = le32(image, 32) as u64;
    let fat_sectors = le32(image, 36) as u64;
    let root_cluster = le32(image, 44);
    if !matches!(sector, 512 | 4096)
        || sectors_per_cluster == 0
        || fats == 0
        || root_entries != 0
        || total16 != 0
        || fat16 != 0
        || fat_sectors == 0
    {
        return Err("not a FAT32 boot sector".into());
    }
    if total * sector != image.len() as u64 {
        return Err(format!(
            "boot sector says {} bytes, the image is {}",
            total * sector,
            image.len()
        ));
    }
    let data_start = reserved + fats * fat_sectors;
    let clusters = ((total - data_start) / sectors_per_cluster) as u32;
    Ok(Geometry {
        sector,
        sectors_per_cluster,
        fat_start: reserved,
        fat_sectors,
        fats,
        data_start,
        root_cluster,
        clusters,
    })
}

impl Geometry {
    fn cluster_sector(&self, cluster: u32) -> u64 {
        self.data_start + (cluster as u64 - 2) * self.sectors_per_cluster
    }

    fn cluster_bytes(&self) -> u64 {
        self.sectors_per_cluster * self.sector
    }

    /// FAT entry `cluster` in copy `fat`, its top four bits masked.
    fn entry(&self, image: &[u8], fat: u64, cluster: u32) -> Result<u32, String> {
        if cluster < 2 || cluster >= self.clusters + 2 {
            return Err(format!("cluster {cluster} out of range"));
        }
        let at = ((self.fat_start + fat * self.fat_sectors) * self.sector) as usize
            + cluster as usize * 4;
        Ok(le32(image, at) & 0x0fff_ffff)
    }

    /// The chain from `first`, identical in every FAT copy.
    fn chain(&self, image: &[u8], first: u32) -> Result<Vec<u32>, String> {
        let mut out = Vec::new();
        let mut c = first;
        loop {
            if out.len() > self.clusters as usize {
                return Err(format!("chain from {first} loops"));
            }
            out.push(c);
            let next = self.entry(image, 0, c)?;
            for fat in 1..self.fats {
                if self.entry(image, fat, c)? != next {
                    return Err(format!("FAT copy {fat} differs at cluster {c}"));
                }
            }
            if next >= 0x0fff_fff8 {
                return Ok(out);
            }
            if next < 2 {
                return Err(format!(
                    "chain from {first} hits free or reserved cluster {next}"
                ));
            }
            c = next;
        }
    }
}

/// A short directory entry: where it is and what it says.
#[derive(Clone, Copy, Debug)]
struct DirEntry {
    sector: u64,
    offset: u32,
    attr: u8,
    first_cluster: u32,
    size: u32,
}

/// The long name of the LFN entries just before a short entry, if any.
fn long_name(lfn: &[[u8; 32]]) -> Option<String> {
    let mut units = Vec::new();
    for e in lfn.iter().rev() {
        for range in [1..11, 14..26, 28..32] {
            for pair in e[range].chunks(2) {
                units.push(u16::from_le_bytes([pair[0], pair[1]]));
            }
        }
    }
    let end = units
        .iter()
        .position(|&u| u == 0 || u == 0xffff)
        .unwrap_or(units.len());
    String::from_utf16(&units[..end]).ok()
}

/// The entries of the directory starting at `cluster`.
fn list(g: &Geometry, image: &[u8], cluster: u32) -> Result<Vec<(String, DirEntry)>, String> {
    let mut out = Vec::new();
    let mut lfn: Vec<[u8; 32]> = Vec::new();
    for c in g.chain(image, cluster)? {
        for s in 0..g.sectors_per_cluster {
            let sector = g.cluster_sector(c) + s;
            let base = (sector * g.sector) as usize;
            for i in 0..(g.sector / 32) as usize {
                let raw: [u8; 32] = image[base + i * 32..base + i * 32 + 32].try_into().unwrap();
                if raw[0] == 0 {
                    return Ok(out);
                }
                if raw[0] == 0xe5 {
                    lfn.clear();
                    continue;
                }
                if raw[11] == 0x0f {
                    lfn.push(raw);
                    continue;
                }
                let short: String = {
                    let base_name = String::from_utf8_lossy(&raw[..8]).trim_end().to_string();
                    let ext = String::from_utf8_lossy(&raw[8..11]).trim_end().to_string();
                    if ext.is_empty() {
                        base_name
                    } else {
                        format!("{base_name}.{ext}")
                    }
                };
                let name = long_name(&lfn).unwrap_or(short);
                lfn.clear();
                let first_cluster = (le16(&raw, 20) as u32) << 16 | le16(&raw, 26) as u32;
                out.push((
                    name,
                    DirEntry {
                        sector,
                        offset: (i * 32) as u32,
                        attr: raw[11],
                        first_cluster,
                        size: le32(&raw, 28),
                    },
                ));
            }
        }
    }
    Ok(out)
}

/// Find `path` (`/`-separated) from the root.
fn find(g: &Geometry, image: &[u8], path: &str) -> Result<DirEntry, String> {
    let mut dir = g.root_cluster;
    let parts: Vec<&str> = path.split('/').collect();
    for (i, part) in parts.iter().enumerate() {
        let entries = list(g, image, dir)?;
        let (_, entry) = entries
            .into_iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(part))
            .ok_or_else(|| format!("{path}: `{part}` not found"))?;
        let is_dir = entry.attr & 0x10 != 0;
        if i + 1 < parts.len() {
            if !is_dir {
                return Err(format!("{path}: `{part}` is not a directory"));
            }
            dir = entry.first_cluster;
        } else {
            if is_dir {
                return Err(format!("{path} is a directory"));
            }
            return Ok(entry);
        }
    }
    unreachable!("a path has at least one part")
}

/// The slot map of a template image: each of [`slot_paths`] found, its size
/// as built, and its chain contiguous in every FAT copy.
pub fn read_back(image: &[u8]) -> Result<SlotMap, String> {
    let g = geometry(image)?;
    let mut entries = Vec::new();
    for (path, size) in slot_paths() {
        entries.push(slot_entry(&g, image, &path, size)?);
    }
    Ok(SlotMap {
        sector_size: g.sector as u32,
        entries,
    })
}

/// One slot file's row: `path` must be `size` bytes in one contiguous chain.
pub fn slot_entry(g: &Geometry, image: &[u8], path: &str, size: u64) -> Result<SlotEntry, String> {
    let e = find(g, image, path)?;
    if e.size as u64 != size {
        return Err(format!("{path} is {} bytes, not {size}", e.size));
    }
    let chain = g.chain(image, e.first_cluster)?;
    if chain.len() as u64 != size.div_ceil(g.cluster_bytes()) {
        return Err(format!("{path}: {} clusters for {size} bytes", chain.len()));
    }
    if let Some(w) = chain.windows(2).find(|w| w[1] != w[0] + 1) {
        return Err(format!(
            "{path} is not contiguous: cluster {} is followed by {}",
            w[0], w[1]
        ));
    }
    Ok(SlotEntry {
        path: path.to_string(),
        first_sector: g.cluster_sector(e.first_cluster),
        sectors: chain.len() as u64 * g.sectors_per_cluster,
        dirent_sector: e.sector,
        dirent_offset: e.offset,
    })
}
