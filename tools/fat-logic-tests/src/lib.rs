// SPDX-License-Identifier: MPL-2.0
//! Host KATs for L3a-2's pure FAT logic, `kernel/src/fs/fat_logic.rs`,
//! mounted through `#[path]`: the boot-sector geometry checks (trap T13) and
//! the two dirty indicators read apart (trap T4), on real FAT12/16/32 images
//! made by the vendored rust-fatfs and then patched by hand (each patch is
//! written out in the test: the patched bytes are the oracle).

#![allow(dead_code)]

#[path = "../../../kernel/src/fs/fat_logic.rs"]
mod fat_logic;

#[cfg(test)]
mod tests {
    use super::fat_logic::*;
    use std::io::Cursor;

    /// A freshly formatted volume of `sectors` 512-byte sectors.
    fn image(fat_type: fatfs::FatType, sectors: u32) -> Vec<u8> {
        let mut disk = Cursor::new(vec![0u8; sectors as usize * 512]);
        fatfs::format_volume(
            &mut fatfs::StdIoWrapper::new(&mut disk),
            fatfs::FormatVolumeOptions::new()
                .fat_type(fat_type)
                .total_sectors(sectors),
        )
        .expect("format");
        disk.into_inner()
    }

    fn fat16() -> Vec<u8> {
        image(fatfs::FatType::Fat16, 64 * 1024)
    }

    fn fat32() -> Vec<u8> {
        image(fatfs::FatType::Fat32, 128 * 1024)
    }

    fn fat12() -> Vec<u8> {
        image(fatfs::FatType::Fat12, 4 * 1024)
    }

    /// Whether the library refuses the image too, without panicking.
    fn library_refuses(img: Vec<u8>) -> bool {
        let r = std::panic::catch_unwind(move || {
            fatfs::FileSystem::new(
                fatfs::StdIoWrapper::new(Cursor::new(img)),
                fatfs::FsOptions::new(),
            )
            .is_err()
        });
        match r {
            Ok(refused) => refused,
            Err(_) => panic!("rust-fatfs panicked on a bad boot sector"),
        }
    }

    #[test]
    fn good_volumes_pass() {
        for (img, fat32) in [(fat12(), false), (fat16(), false), (fat32(), true)] {
            let bpb = check_bpb(&img[..512]).expect("sane");
            assert_eq!(bpb.bytes_per_sector, 512);
            assert_eq!(bpb.fat32, fat32);
            assert!(bpb.reserved_sectors >= 1);
            assert!(!library_refuses(img));
        }
    }

    /// T13: one patched field at a time, on FAT16 and FAT32, each refused
    /// with its own reason before the library sees it.
    #[test]
    fn bad_geometry_is_refused_field_by_field() {
        type Patch = fn(&mut [u8]);
        let cases: &[(&str, Patch, BpbError)] = &[
            (
                "bytes/sector 0",
                |s| s[11..13].copy_from_slice(&0u16.to_le_bytes()),
                BpbError::BytesPerSector,
            ),
            (
                "bytes/sector 256",
                |s| s[11..13].copy_from_slice(&256u16.to_le_bytes()),
                BpbError::BytesPerSector,
            ),
            (
                "bytes/sector 768",
                |s| s[11..13].copy_from_slice(&768u16.to_le_bytes()),
                BpbError::BytesPerSector,
            ),
            (
                "bytes/sector 8192",
                |s| s[11..13].copy_from_slice(&8192u16.to_le_bytes()),
                BpbError::BytesPerSector,
            ),
            (
                "sectors/cluster 0",
                |s| s[13] = 0,
                BpbError::SectorsPerCluster,
            ),
            (
                "sectors/cluster 3",
                |s| s[13] = 3,
                BpbError::SectorsPerCluster,
            ),
            (
                "reserved 0",
                |s| s[14..16].copy_from_slice(&0u16.to_le_bytes()),
                BpbError::ReservedSectors,
            ),
            ("fats 0", |s| s[16] = 0, BpbError::FatCount),
            ("fats 3", |s| s[16] = 3, BpbError::FatCount),
            (
                "no total sectors",
                |s| {
                    s[19..21].copy_from_slice(&0u16.to_le_bytes());
                    s[32..36].copy_from_slice(&0u32.to_le_bytes());
                },
                BpbError::TotalSectors,
            ),
            (
                "two different totals",
                |s| {
                    s[19..21].copy_from_slice(&1000u16.to_le_bytes());
                    s[32..36].copy_from_slice(&2000u32.to_le_bytes());
                },
                BpbError::TotalSectors,
            ),
            ("no signature", |s| s[510] = 0, BpbError::NotABootSector),
            (
                "exFAT",
                |s| s[3..11].copy_from_slice(b"EXFAT   "),
                BpbError::ExFat,
            ),
        ];
        for img in [fat16(), fat32()] {
            for (what, patch, want) in cases {
                let mut bad = img.clone();
                patch(&mut bad[..512]);
                assert_eq!(check_bpb(&bad[..512]), Err(*want), "{what}");
            }
        }
    }

    /// T13, the layout-specific fields.
    #[test]
    fn bad_layout_fields_are_refused() {
        // FAT16: root entries 0, root entries not filling whole sectors,
        // sectors/FAT 0, and FATs that eat the whole volume.
        let img = fat16();
        let mut b = img.clone();
        b[17..19].copy_from_slice(&0u16.to_le_bytes());
        // 0 root entries reads as FAT32's shape only if sectors/FAT16 is 0 too.
        assert_eq!(check_bpb(&b[..512]), Err(BpbError::RootEntries));
        let mut b = img.clone();
        b[17..19].copy_from_slice(&17u16.to_le_bytes());
        assert_eq!(check_bpb(&b[..512]), Err(BpbError::RootEntries));
        let mut b = img.clone();
        b[22..24].copy_from_slice(&0u16.to_le_bytes());
        // With sectors/FAT16 0 the shape is FAT32's, whose sectors/FAT32 is 0
        // on a FAT16 sector (bytes 36..40 are the drive number and such).
        assert!(matches!(
            check_bpb(&b[..512]),
            Err(BpbError::SectorsPerFat | BpbError::RootEntries)
        ));
        let mut b = img.clone();
        b[22..24].copy_from_slice(&0xffffu16.to_le_bytes());
        assert_eq!(check_bpb(&b[..512]), Err(BpbError::NoDataRegion));
        // FAT32: a non-zero root entry count, sectors/FAT 0, and FATs that
        // eat the volume.
        let img = fat32();
        let mut b = img.clone();
        b[17..19].copy_from_slice(&512u16.to_le_bytes());
        assert_eq!(check_bpb(&b[..512]), Err(BpbError::RootEntries));
        let mut b = img.clone();
        b[36..40].copy_from_slice(&0u32.to_le_bytes());
        assert_eq!(check_bpb(&b[..512]), Err(BpbError::SectorsPerFat));
        let mut b = img.clone();
        b[36..40].copy_from_slice(&0x0010_0000u32.to_le_bytes());
        assert_eq!(check_bpb(&b[..512]), Err(BpbError::NoDataRegion));
    }

    /// The library, given the same bad sectors, refuses them too and never
    /// panics: our check is the first line, not the only one.
    #[test]
    fn the_library_also_refuses_without_panicking() {
        let patches: &[fn(&mut [u8])] = &[
            |s| s[11..13].copy_from_slice(&768u16.to_le_bytes()),
            |s| s[13] = 0,
            |s| s[13] = 3,
            |s| s[14..16].copy_from_slice(&0u16.to_le_bytes()),
            |s| s[16] = 0,
            |s| {
                s[19..21].copy_from_slice(&0u16.to_le_bytes());
                s[32..36].copy_from_slice(&0u32.to_le_bytes());
            },
        ];
        for img in [fat16(), fat32()] {
            for patch in patches {
                let mut bad = img.clone();
                patch(&mut bad[..512]);
                assert!(check_bpb(&bad[..512]).is_err());
                assert!(library_refuses(bad));
            }
        }
    }

    fn first_fat_sector(img: &[u8]) -> &[u8] {
        let reserved = u16::from_le_bytes([img[14], img[15]]) as usize;
        &img[reserved * 512..reserved * 512 + 512]
    }

    /// T4 (Chad's review): the four fixtures, each patched by hand, on FAT16
    /// and FAT32. The two indicators are read apart; rust-fatfs's own
    /// `read_status_flags` merges them (dirty if either), which is why the
    /// mount reads them itself.
    #[test]
    fn the_two_dirty_indicators_are_read_apart() {
        for (img, fat32) in [(fat16(), false), (fat32(), true)] {
            let status_at = if fat32 { 0x41 } else { 0x25 };
            let reserved = u16::from_le_bytes([img[14], img[15]]) as usize;
            // FAT[1]'s clean-shutdown bit: bit 27 of the 4-byte entry at 4
            // (byte 7, bit 3), or bit 15 of the 2-byte entry at 2 (byte 3,
            // bit 7).
            let (clean_byte, clean_bit) = if fat32 {
                (reserved * 512 + 7, 0x08)
            } else {
                (reserved * 512 + 3, 0x80)
            };
            let fixtures = [(false, false), (true, false), (false, true), (true, true)];
            for (bpb_dirty, fat1_dirty) in fixtures {
                let mut v = img.clone();
                if bpb_dirty {
                    v[status_at] |= 1;
                }
                if fat1_dirty {
                    v[clean_byte] &= !clean_bit;
                }
                let bpb = check_bpb(&v[..512]).expect("sane");
                let got = dirty_flags(&bpb, &v[..512], first_fat_sector(&v), !fat32);
                assert_eq!(
                    got,
                    Dirty {
                        bpb: bpb_dirty,
                        fat1: fat1_dirty
                    },
                    "fat32={fat32} bpb={bpb_dirty} fat1={fat1_dirty}"
                );
                let fs =
                    fatfs::FileSystem::new(Cursor::new(v), fatfs::FsOptions::new()).expect("mount");
                assert_eq!(
                    fs.read_status_flags().unwrap().dirty(),
                    bpb_dirty || fat1_dirty
                );
            }
            // The hard-error bit alone also counts as FAT[1] dirty.
            let mut v = img.clone();
            let (err_byte, err_bit) = if fat32 {
                (reserved * 512 + 7, 0x04)
            } else {
                (reserved * 512 + 3, 0x40)
            };
            v[err_byte] &= !err_bit;
            let bpb = check_bpb(&v[..512]).unwrap();
            assert!(dirty_flags(&bpb, &v[..512], first_fat_sector(&v), !fat32).fat1);
        }
        // FAT12 has no FAT[1] flags.
        let img = fat12();
        let bpb = check_bpb(&img[..512]).unwrap();
        assert_eq!(
            dirty_flags(&bpb, &img[..512], first_fat_sector(&img), false),
            Dirty::default()
        );
    }

    /// Chad's #18 review: a FAT whose chain loops (`A -> B -> A`) is
    /// corruption, and reading must say so instead of returning the same
    /// clusters again (a file) or walking forever (a directory, which has no
    /// size to stop at). Real FAT16 images, patched by hand.
    fn fat16_entry_at(img: &[u8], cluster: u32) -> usize {
        let reserved = u16::from_le_bytes([img[14], img[15]]) as usize;
        reserved * 512 + cluster as usize * 2
    }

    fn set_fat16(img: &mut [u8], cluster: u32, value: u16) {
        let reserved = u16::from_le_bytes([img[14], img[15]]) as usize;
        let per_fat = u16::from_le_bytes([img[22], img[23]]) as usize * 512;
        let fats = img[16] as usize;
        for f in 0..fats {
            let at = reserved * 512 + f * per_fat + cluster as usize * 2;
            img[at..at + 2].copy_from_slice(&value.to_le_bytes());
        }
    }

    fn run_bounded<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(f());
        });
        rx.recv_timeout(std::time::Duration::from_secs(20))
            .expect("the walk did not finish: a loop was followed")
    }

    #[test]
    fn a_looping_chain_is_corruption() {
        use std::io::{Read as _, Write as _};
        // A file of three clusters, then its chain patched to A -> B -> A.
        let mut img = fat16();
        let cluster_size = {
            let mut disk = Cursor::new(&mut img);
            let fs = fatfs::FileSystem::new(
                fatfs::StdIoWrapper::new(&mut disk),
                fatfs::FsOptions::new(),
            )
            .expect("mount");
            let cs = fs.cluster_size() as usize;
            let mut f = fs.root_dir().create_file("loop.bin").unwrap();
            f.write_all(&vec![0x5a; cs * 3]).unwrap();
            f.flush().unwrap();
            drop(f);
            // A directory whose first cluster is full (8.3 names, one entry
            // each, after `.` and `..`): no end marker in it, so listing it
            // has to follow its chain.
            let dir = fs.root_dir().create_dir("loopdir").unwrap();
            for i in 0..cs / 32 - 2 {
                dir.create_file(&format!("F{i:04}.TXT")).unwrap();
            }
            cs
        };
        // The first clusters, from the root directory's raw short entries
        // (FAT16: the root follows the FATs; first cluster at byte 26).
        let short_first = |img: &[u8], name: &[u8; 11]| -> u32 {
            let reserved = u16::from_le_bytes([img[14], img[15]]) as usize;
            let per_fat = u16::from_le_bytes([img[22], img[23]]) as usize;
            let root = (reserved + img[16] as usize * per_fat) * 512;
            let entries = u16::from_le_bytes([img[17], img[18]]) as usize;
            (0..entries)
                .map(|i| &img[root + i * 32..root + i * 32 + 32])
                .find(|e| &e[0..11] == name)
                .map(|e| u32::from(u16::from_le_bytes([e[26], e[27]])))
                .expect("short entry")
        };
        let file_first = short_first(&img, b"LOOP    BIN");
        let dir_first = short_first(&img, b"LOOPDIR    ");
        // The chain as written: A -> A+1 -> A+2 -> end. Make it A -> A+1 -> A.
        let a = file_first;
        let b = u16::from_le_bytes([
            img[fat16_entry_at(&img, a)],
            img[fat16_entry_at(&img, a) + 1],
        ]);
        assert_eq!(u32::from(b), a + 1);
        set_fat16(&mut img, a + 1, a as u16);
        // The directory: its first cluster pointing at itself.
        set_fat16(&mut img, dir_first, dir_first as u16);
        // Every 32-byte slot of that cluster is in use (no end marker).
        let data_start = {
            let reserved = u16::from_le_bytes([img[14], img[15]]) as usize;
            let per_fat = u16::from_le_bytes([img[22], img[23]]) as usize;
            let root_sectors = u16::from_le_bytes([img[17], img[18]]) as usize * 32 / 512;
            (reserved + img[16] as usize * per_fat + root_sectors) * 512
        };
        let dir_at = data_start + (dir_first as usize - 2) * cluster_size;
        assert!((0..cluster_size / 32).all(|i| img[dir_at + i * 32] != 0));

        let file_result = {
            let img = img.clone();
            run_bounded(move || {
                let mut disk = Cursor::new(img);
                let fs = fatfs::FileSystem::new(
                    fatfs::StdIoWrapper::new(&mut disk),
                    fatfs::FsOptions::new(),
                )
                .unwrap();
                let mut f = fs.root_dir().open_file("loop.bin").unwrap();
                let mut out = Vec::new();
                f.read_to_end(&mut out).map(|_| out.len())
            })
        };
        let err = file_result.expect_err("a looping file chain read back as data");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData, "{err}");

        let dir_result = run_bounded(move || {
            let mut disk = Cursor::new(img);
            let fs = fatfs::FileSystem::new(
                fatfs::StdIoWrapper::new(&mut disk),
                fatfs::FsOptions::new(),
            )
            .unwrap();
            let dir = fs.root_dir().open_dir("loopdir").unwrap();
            let mut n = 0usize;
            for e in dir.iter() {
                if e.is_err() {
                    return Err(n);
                }
                n += 1;
            }
            Ok(n)
        });
        assert!(
            dir_result.is_err(),
            "a looping directory chain was walked: {dir_result:?}"
        );
    }
}
