// SPDX-License-Identifier: MPL-2.0
//! OT-87 R3: the GPT writer and reader (`gpt_logic.rs`) against the hand-laid
//! oracle (`gpt_oracle.rs`, from `fixtures/gpt_oracle.py`), and against a
//! second, independent reader: the `gpt` crate (MIT, v4.1.0, Quyzi/gpt
//! `73086def`; THINK §11.2).

use std::collections::BTreeMap;
use std::io::{self, Read, Seek, SeekFrom, Write};

use super::gpt_logic::{
    self, ATTR_INCOMPLETE, Disk, ESP_TYPE_GUID, LAYOUT_ARRAY_BYTES, LOGANSOS_NEOFS_TYPE_GUID,
    Layout, gpt_layout,
};
use super::gpt_oracle::{self, DISK_GUID, ESP_GUID, NEOFS_GUID};

/// A sparse disk image in 4 KiB pages: unwritten bytes read as zero.
#[derive(Debug, Default)]
pub struct Sparse {
    pub capacity: u64,
    pub sector: u64,
    pages: BTreeMap<u64, Vec<u8>>,
    pos: u64,
}

const PAGE: u64 = 4096;

impl Sparse {
    pub fn new(capacity: u64, sector: u64) -> Self {
        Self {
            capacity,
            sector,
            ..Self::default()
        }
    }

    pub fn put(&mut self, offset: u64, data: &[u8]) {
        for (i, &b) in data.iter().enumerate() {
            let at = offset + i as u64;
            let page = self
                .pages
                .entry(at / PAGE)
                .or_insert_with(|| vec![0; PAGE as usize]);
            page[(at % PAGE) as usize] = b;
        }
    }

    pub fn get(&self, offset: u64, out: &mut [u8]) {
        for (i, b) in out.iter_mut().enumerate() {
            let at = offset + i as u64;
            *b = self
                .pages
                .get(&(at / PAGE))
                .map_or(0, |p| p[(at % PAGE) as usize]);
        }
    }

    /// The disk [`gpt_layout`] describes: MBR, both headers, both arrays.
    pub fn with_layout(capacity: u64, layout: &Layout) -> Self {
        let s = layout.sector_size;
        let mut img = Self::new(capacity, s);
        img.put(0, &layout.mbr);
        img.put(s, &layout.primary_header);
        img.put(2 * s, &layout.entries);
        img.put(layout.backup_entries_lba * s, &layout.entries);
        img.put(layout.last_lba * s, &layout.backup_header);
        img
    }
}

impl Disk for Sparse {
    fn capacity(&self) -> u64 {
        self.capacity
    }

    fn sector_size(&self) -> u64 {
        self.sector
    }

    fn read(&self, offset: u64, out: &mut [u8]) -> bool {
        if offset + out.len() as u64 > self.capacity {
            return false;
        }
        self.get(offset, out);
        true
    }
}

impl Read for Sparse {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = (self.capacity.saturating_sub(self.pos) as usize).min(buf.len());
        let pos = self.pos;
        self.get(pos, &mut buf[..n]);
        self.pos += n as u64;
        Ok(n)
    }
}

impl Write for Sparse {
    fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("read-only test image"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Seek for Sparse {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        self.pos = match to {
            SeekFrom::Start(p) => p,
            SeekFrom::End(d) => self.capacity.checked_add_signed(d).unwrap(),
            SeekFrom::Current(d) => self.pos.checked_add_signed(d).unwrap(),
        };
        Ok(self.pos)
    }
}

const GIB4: u64 = 4 * 1024 * 1024 * 1024;

/// `sparse` as a full `len`-byte buffer: zero except the listed bytes.
fn expand(sparse: &[(usize, u8)], len: usize) -> Vec<u8> {
    let mut out = vec![0u8; len];
    for &(at, b) in sparse {
        out[at] = b;
    }
    out
}

struct Oracle {
    s: u64,
    l: u64,
    first: u64,
    last: u64,
    esp: (u64, u64),
    neofs: (u64, u64),
    blocks: u64,
    crcs: (u32, u32, u32),
    mbr: &'static [(usize, u8)],
    primary: &'static [(usize, u8)],
    backup: &'static [(usize, u8)],
    e1: &'static [(usize, u8)],
    e2: &'static [(usize, u8)],
}

fn oracles() -> [Oracle; 2] {
    use gpt_oracle::{s512 as a, s4096 as b};
    [
        Oracle {
            s: a::S,
            l: a::L,
            first: a::FIRST,
            last: a::LAST,
            esp: a::ESP,
            neofs: a::NEOFS,
            blocks: a::BLOCKS,
            crcs: (a::CRC_E, a::CRC_H1, a::CRC_H2),
            mbr: a::MBR,
            primary: a::PRIMARY_HEADER,
            backup: a::BACKUP_HEADER,
            e1: a::ENTRY1,
            e2: a::ENTRY2,
        },
        Oracle {
            s: b::S,
            l: b::L,
            first: b::FIRST,
            last: b::LAST,
            esp: b::ESP,
            neofs: b::NEOFS,
            blocks: b::BLOCKS,
            crcs: (b::CRC_E, b::CRC_H1, b::CRC_H2),
            mbr: b::MBR,
            primary: b::PRIMARY_HEADER,
            backup: b::BACKUP_HEADER,
            e1: b::ENTRY1,
            e2: b::ENTRY2,
        },
    ]
}

fn layout_for(o: &Oracle) -> Layout {
    gpt_layout(o.s, o.l, DISK_GUID, ESP_GUID, NEOFS_GUID, false).expect("4 GiB fits")
}

/// The oracle's geometry is the THINK's §4.4 numbers, written out by hand.
#[test]
fn oracle_geometry_is_the_think_table() {
    let [a, b] = oracles();
    assert_eq!((a.s, a.l, a.first, a.last), (512, 8_388_607, 34, 8_388_574));
    assert_eq!(a.esp, (2048, 1_050_623));
    assert_eq!(a.neofs, (1_050_624, 8_386_559));
    assert_eq!((b.s, b.l, b.first, b.last), (4096, 1_048_575, 6, 1_048_570));
    assert_eq!(b.esp, (256, 131_327));
    assert_eq!(b.neofs, (131_328, 1_048_319));
    assert_eq!((a.blocks, b.blocks), (916_992, 916_992));
}

/// Check 1: the writer's bytes equal the oracle's, every byte of LBA 0, both
/// headers and the whole entry array (entries 3..128 all zero).
#[test]
fn writer_matches_the_hand_laid_oracle() {
    for o in oracles() {
        let layout = layout_for(&o);
        let s = o.s as usize;
        assert_eq!(layout.mbr, expand(o.mbr, s), "S={} MBR", o.s);
        assert_eq!(
            layout.primary_header,
            expand(o.primary, s),
            "S={} primary",
            o.s
        );
        assert_eq!(
            layout.backup_header,
            expand(o.backup, s),
            "S={} backup",
            o.s
        );
        let mut entries = expand(o.e1, 128);
        entries.extend(expand(o.e2, 128));
        entries.resize(LAYOUT_ARRAY_BYTES, 0);
        assert_eq!(layout.entries, entries, "S={} entries", o.s);
        assert_eq!(layout.esp, o.esp);
        assert_eq!(layout.neofs, o.neofs);
        assert_eq!(layout.neofs_total_blocks, o.blocks);
        assert_eq!(layout.backup_entries_lba, o.last + 1);
        // The three CRCs, named separately, are the oracle's (zlib.crc32).
        let (e, h1, h2) = o.crcs;
        let field = |h: &[u8], at: usize| u32::from_le_bytes(h[at..at + 4].try_into().unwrap());
        assert_eq!(field(&layout.primary_header, 88), e);
        assert_eq!(field(&layout.backup_header, 88), e);
        assert_eq!(field(&layout.primary_header, 16), h1);
        assert_eq!(field(&layout.backup_header, 16), h2);
        assert_ne!(h1, h2, "MyLBA, AlternateLBA and the array LBA differ");
    }
}

/// Check 2: the shared reader (the guard's) believes the table, from the
/// primary, and from the backup alone when the primary is gone.
#[test]
fn shared_reader_accepts_the_oracle_disk() {
    for o in oracles() {
        let layout = layout_for(&o);
        let mut img = Sparse::with_layout(GIB4, &layout);
        for copy in ["primary", "backup"] {
            let gpt = gpt_logic::read(&img).unwrap_or_else(|| panic!("S={} {copy}", o.s));
            assert_eq!(gpt.first_usable, o.first);
            assert_eq!(gpt.last_usable, o.last);
            assert_eq!(gpt.disk_guid, DISK_GUID);
            assert_eq!(gpt.entries.len(), 2);
            let (e1, e2) = (gpt.entries[0], gpt.entries[1]);
            assert_eq!(
                (e1.index, e1.type_guid, e1.unique_guid),
                (1, ESP_TYPE_GUID, ESP_GUID)
            );
            assert_eq!(
                (e1.first_lba, e1.last_lba, e1.attributes),
                (o.esp.0, o.esp.1, 0)
            );
            assert_eq!(
                (e2.index, e2.type_guid, e2.unique_guid),
                (2, LOGANSOS_NEOFS_TYPE_GUID, NEOFS_GUID)
            );
            assert_eq!(
                (e2.first_lba, e2.last_lba, e2.attributes),
                (o.neofs.0, o.neofs.1, 0)
            );
            if copy == "primary" {
                assert_eq!(gpt.header_lba, 1);
                img.put(o.s, &vec![0u8; o.s as usize]);
            } else {
                assert_eq!(gpt.header_lba, o.l);
            }
        }
    }
}

/// Check 3: an independent reader agrees, the `gpt` crate opening the same
/// image read-only.
#[test]
fn the_gpt_crate_agrees() {
    for o in oracles() {
        let layout = layout_for(&o);
        let img = Sparse::with_layout(GIB4, &layout);
        let lb = if o.s == 512 {
            gpt::disk::LogicalBlockSize::Lb512
        } else {
            gpt::disk::LogicalBlockSize::Lb4096
        };
        let disk = gpt::GptConfig::new()
            .writable(false)
            .logical_block_size(lb)
            .open_from_device(img)
            .unwrap_or_else(|e| panic!("S={}: the gpt crate refused: {e:?}", o.s));
        let h = disk.primary_header().expect("primary");
        assert_eq!((h.first_usable, h.last_usable), (o.first, o.last));
        let b = disk.backup_header().expect("backup");
        assert_eq!((b.current_lba, b.backup_lba), (o.l, 1));
        let parts = disk.partitions();
        assert_eq!(parts.len(), 2, "S={}: {parts:?}", o.s);
        let esp = &parts[&1];
        assert_eq!((esp.first_lba, esp.last_lba), o.esp);
        assert_eq!(
            esp.part_type_guid.guid,
            uuid::uuid!("c12a7328-f81f-11d2-ba4b-00a0c93ec93b")
        );
        assert_eq!(esp.name, "EFI System");
        let neofs = &parts[&2];
        assert_eq!((neofs.first_lba, neofs.last_lba), o.neofs);
        assert_eq!(
            neofs.part_type_guid.guid,
            uuid::uuid!("eaa7a9d0-75c1-4385-9664-b2a74fc2622d")
        );
        assert_eq!(neofs.name, "LogansOS");
        assert_eq!(neofs.flags, 0);
    }
}

/// INCOMPLETE is bit 48 of entry 2's attributes, and setting it changes CRC-E
/// and therefore both header CRCs; the reader reports it.
#[test]
fn incomplete_is_bit_48_and_moves_every_crc() {
    for o in oracles() {
        let done = layout_for(&o);
        let mut busy =
            gpt_layout(o.s, o.l, DISK_GUID, ESP_GUID, NEOFS_GUID, true).expect("4 GiB fits");
        assert_eq!(
            &busy.entries[128 + 48..128 + 56],
            &(1u64 << 48).to_le_bytes()
        );
        let mut diff: Vec<usize> = (0..LAYOUT_ARRAY_BYTES)
            .filter(|&i| busy.entries[i] != done.entries[i])
            .collect();
        assert_eq!(diff, vec![128 + 54], "only byte 6 of the attributes");
        diff = (0..o.s as usize)
            .filter(|&i| busy.primary_header[i] != done.primary_header[i])
            .collect();
        assert!(
            diff.iter()
                .all(|i| (16..20).contains(i) || (88..92).contains(i))
        );
        let gpt = gpt_logic::read(&Sparse::with_layout(GIB4, &busy)).expect("valid");
        assert_eq!(gpt.entries[1].attributes, ATTR_INCOMPLETE);
        // A one-byte change without re-sealing is not believed.
        busy.entries[128 + 54] = 0;
        assert!(gpt_logic::read(&Sparse::with_layout(GIB4, &busy)).is_none());
    }
}

/// The writer refuses what §4.4 refuses: other sector sizes, and a disk with
/// no room for NeoFS after the 512 MiB ESP.
#[test]
fn writer_refuses_bad_geometry() {
    let g = |s, l| gpt_layout(s, l, DISK_GUID, ESP_GUID, NEOFS_GUID, false);
    assert!(g(1024, 8_388_607).is_none());
    assert!(g(520, 8_388_607).is_none());
    // 513 MiB of 512 B sectors: the ESP fits, NeoFS does not.
    assert!(g(512, 513 * 2048 - 1).is_none());
    // One MiB of NeoFS past the ESP, plus the backup array: the smallest.
    let l = 514 * 2048 + 32;
    let small = g(512, l).expect("the smallest disk");
    assert_eq!(small.neofs, (513 * 2048, 514 * 2048 - 1));
    assert_eq!(small.neofs_total_blocks, 256);
    assert!(g(512, l - 1).is_none());
}

/// The CRC is CRC-32/ISO-HDLC: the standard check value.
#[test]
fn crc32_check_value() {
    assert_eq!(gpt_logic::crc32(b"123456789"), 0xcbf4_3926);
}
