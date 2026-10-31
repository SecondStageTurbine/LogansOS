// SPDX-License-Identifier: MPL-2.0
//! The VM image layout (OT-87 R1): the rules `check_vm_layout` holds
//! `boot.raw` and `data.raw` to, each against a disk that breaks it, the ESP
//! type GUID against its spec text, and the firmware lookup.

use std::fs;
use std::path::{Path, PathBuf};

use super::*;

/// A boot disk laid out by hand: a protective MBR (entry 0 type 0xee, the
/// 0x55aa signature), the GPT header at LBA 1 pointing at a 4-entry array at
/// LBA 2, the EFI system partition in entry 1, and an ISO9660 volume
/// descriptor at 0x8001 like xorriso's. 64 KiB, so no NeoFS offset past 0
/// is inside it.
fn boot_disk() -> Vec<u8> {
    let mut disk = vec![0u8; 64 * 1024];
    disk[446 + 4] = 0xee;
    disk[510] = 0x55;
    disk[511] = 0xaa;
    disk[512..520].copy_from_slice(b"EFI PART");
    disk[512 + 72..512 + 80].copy_from_slice(&2u64.to_le_bytes());
    disk[512 + 80..512 + 84].copy_from_slice(&4u32.to_le_bytes());
    disk[512 + 84..512 + 88].copy_from_slice(&128u32.to_le_bytes());
    let esp_entry = 2 * 512 + 128;
    disk[esp_entry..esp_entry + 16].copy_from_slice(&ESP_TYPE_GUID);
    disk[0x8001..0x8006].copy_from_slice(b"CD001");
    disk
}

/// A data disk of the expected size with the magic at all three copies.
fn data_disk() -> Vec<u8> {
    let mut disk = vec![0u8; (DATA_DISK_MIB * 1024 * 1024) as usize];
    for offset in NEOFS_SUPERBLOCK_OFFSETS {
        let at = offset as usize;
        disk[at..at + 8].copy_from_slice(NEOFS_MAGIC);
    }
    disk
}

#[test]
fn a_well_formed_pair_passes() {
    let boot = boot_disk();
    check_vm_layout(&boot, &boot.clone(), &data_disk()).unwrap();
}

#[test]
fn the_boot_disk_must_be_the_isos_bytes() {
    let boot = boot_disk();
    let mut iso = boot.clone();
    iso[0x8010] ^= 1;
    let err = check_vm_layout(&boot, &iso, &data_disk()).unwrap_err();
    assert!(err.contains("not the ISO's bytes"), "{err}");
}

#[test]
fn the_boot_disk_must_carry_no_neofs_superblock() {
    // Otherwise the kernel's first-valid-device mount would take it.
    let mut boot = boot_disk();
    boot[..8].copy_from_slice(NEOFS_MAGIC);
    let err = check_vm_layout(&boot, &boot.clone(), &data_disk()).unwrap_err();
    assert!(err.contains("would mount the boot disk"), "{err}");
}

#[test]
fn the_boot_disk_must_have_a_protective_mbr_gpt_and_esp() {
    let data = data_disk();
    let mut no_sig = boot_disk();
    no_sig[511] = 0;
    let mut not_protective = boot_disk();
    not_protective[446 + 4] = 0x0c;
    let mut no_gpt = boot_disk();
    no_gpt[512] = b'X';
    let mut no_esp = boot_disk();
    no_esp[2 * 512 + 128] ^= 0xff;
    for (disk, want) in [
        (no_sig, "no MBR boot signature"),
        (not_protective, "not the protective 0xee"),
        (no_gpt, "no GPT header"),
        (no_esp, "no EFI system partition"),
    ] {
        let err = check_vm_layout(&disk, &disk.clone(), &data).unwrap_err();
        assert!(err.contains(want), "{want}: {err}");
    }
}

#[test]
fn the_boot_disk_must_be_whole_sectors() {
    let mut boot = boot_disk();
    boot.push(0);
    let err = check_vm_layout(&boot, &boot.clone(), &data_disk()).unwrap_err();
    assert!(err.contains("512-byte sectors"), "{err}");
}

#[test]
fn the_data_disk_must_be_neofs_at_every_copy_and_its_size() {
    let boot = boot_disk();
    for offset in NEOFS_SUPERBLOCK_OFFSETS {
        let mut data = data_disk();
        data[offset as usize] = 0;
        let err = check_vm_layout(&boot, &boot, &data).unwrap_err();
        assert!(
            err.contains(&format!("no NeoFS superblock at {offset:#x}")),
            "{err}"
        );
    }
    let mut short = data_disk();
    short.truncate(short.len() - 4096);
    let err = check_vm_layout(&boot, &boot, &short).unwrap_err();
    assert!(err.contains("not 134217728"), "{err}");
}

#[test]
fn the_esp_type_guid_is_the_spec_text_in_gpt_byte_order() {
    // C12A7328-F81F-11D2-BA4B-00A0C93EC93B: fields 1-3 are stored
    // little-endian, fields 4-5 byte for byte.
    let text = "C12A7328-F81F-11D2-BA4B-00A0C93EC93B";
    let fields: Vec<Vec<u8>> = text
        .split('-')
        .map(|f| {
            (0..f.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&f[i..i + 2], 16).unwrap())
                .collect()
        })
        .collect();
    let mut want: Vec<u8> = Vec::new();
    for (i, field) in fields.iter().enumerate() {
        if i < 3 {
            want.extend(field.iter().rev());
        } else {
            want.extend(field);
        }
    }
    assert_eq!(want, ESP_TYPE_GUID);
}

#[test]
fn the_neofs_offsets_are_the_kernels() {
    assert_eq!(NEOFS_MAGIC, b"NEOFSv1\0");
    assert_eq!(NEOFS_SUPERBLOCK_OFFSETS, [0, 1 << 20, 64 << 20]);
    // The last copy must fit on the data disk.
    assert!(NEOFS_SUPERBLOCK_OFFSETS[2] + 512 <= DATA_DISK_MIB << 20);
}

/// A scratch directory per test, holding the firmware pair when `files`.
fn firmware_dir(tag: &str, files: bool) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("xtask-ovmf-{tag}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    if files {
        fs::write(dir.join(OVMF_CODE), b"code").unwrap();
        fs::write(dir.join(OVMF_VARS), b"vars").unwrap();
    }
    dir
}

/// A digest that reports each file's pin when its bytes are `code`/`vars`.
fn pinned_digest(path: &Path) -> Result<String, String> {
    Ok(match fs::read(path).unwrap().as_slice() {
        b"code" => OVMF_CODE_BLAKE3.to_string(),
        b"vars" => OVMF_VARS_BLAKE3.to_string(),
        _ => "0".repeat(64),
    })
}

#[test]
fn the_firmware_is_the_first_directory_holding_both_files() {
    let empty = firmware_dir("empty", false);
    let full = firmware_dir("full", true);
    let (ovmf, warning) = find_ovmf(&[empty.clone(), full.clone()], true, pinned_digest).unwrap();
    assert_eq!(ovmf.code, full.join(OVMF_CODE));
    assert_eq!(ovmf.vars, full.join(OVMF_VARS));
    assert_eq!(warning, None);
    let err = find_ovmf(std::slice::from_ref(&empty), false, pinned_digest).unwrap_err();
    assert!(err.starts_with("INFRA: no OVMF found"), "{err}");
    assert!(err.contains(&empty.display().to_string()), "{err}");
    let _ = fs::remove_dir_all(empty);
    let _ = fs::remove_dir_all(full);
}

#[test]
fn an_unpinned_firmware_fails_the_gate_and_warns_elsewhere() {
    let dir = firmware_dir("unpinned", true);
    fs::write(dir.join(OVMF_VARS), b"other vars").unwrap();
    let err = find_ovmf(std::slice::from_ref(&dir), true, pinned_digest).unwrap_err();
    assert!(
        err.starts_with("INFRA:") && err.contains("not the pinned build"),
        "{err}"
    );
    let (_, warning) = find_ovmf(std::slice::from_ref(&dir), false, pinned_digest).unwrap();
    assert!(warning.unwrap().contains(OVMF_VARS));
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn the_env_directory_is_searched_first() {
    let dirs = ovmf_search_dirs(Some(Path::new("/opt/qemu/share")));
    assert_eq!(dirs[0], PathBuf::from("/opt/qemu/share"));
    assert_eq!(&dirs[1..], OVMF_DEFAULT_DIRS.map(PathBuf::from));
    assert_eq!(ovmf_search_dirs(None), OVMF_DEFAULT_DIRS.map(PathBuf::from));
}

#[test]
fn the_vm_boots_from_the_boot_disk_with_no_cdrom() {
    let args = vm_disk_args(
        Path::new("/fw/code.fd"),
        Path::new("/run/vars.fd"),
        Path::new("/vm/boot.raw"),
        Path::new("/vm/data.raw"),
    );
    assert!(!args.iter().any(|a| a.contains("cdrom")));
    let joined = args.join(" ");
    let at = |needle: &str| joined.find(needle).unwrap_or_else(|| panic!("{needle}"));
    assert!(joined.contains("unit=0,readonly=on,file=/fw/code.fd"));
    assert!(joined.contains("if=pflash,format=raw,unit=1,file=/run/vars.fd"));
    // The boot disk is attached first and is the only one in the boot order.
    assert!(at("file=/vm/boot.raw") < at("file=/vm/data.raw"));
    assert_eq!(joined.matches("bootindex=").count(), 1);
    assert!(joined.contains("drive=vmboot,bootindex=0"));
}

// === OT-87 R3: the one-disk image ===

/// The one-disk GUIDs are the oracle's (gpt_oracle.py), written out from
/// their text form by hand: the first three fields little-endian.
#[test]
fn one_disk_guids_are_the_oracles() {
    // 4c6f6761-6e4f-5321-8000-000000000001
    assert_eq!(
        ONE_DISK_GUID,
        [
            0x61, 0x67, 0x6f, 0x4c, 0x4f, 0x6e, 0x21, 0x53, 0x80, 0, 0, 0, 0, 0, 0, 0x01
        ]
    );
    assert_eq!(ONE_DISK_ESP_GUID[15], 0xe5);
    assert_eq!(ONE_DISK_NEOFS_GUID[15], 0xf5);
    assert_eq!(ESP_TYPE_GUID, crate::gpt_logic::ESP_TYPE_GUID);
}

/// The fixed 4 GiB layout is the §4.4 oracle's S=512 geometry.
#[test]
fn one_disk_layout_is_the_think_geometry() {
    let l = one_disk_layout(ONE_DISK_SECTOR);
    assert_eq!((l.sector_size, l.last_lba), (512, 8_388_607));
    assert_eq!(l.esp, (2048, 1_050_623));
    assert_eq!(l.neofs, (1_050_624, 8_386_559));
    assert_eq!(l.neofs_total_blocks, 916_992);
    assert_eq!(
        l.neofs_total_blocks * 4096 % (1 << 20),
        0,
        "a whole MiB volume"
    );
}

/// A sparse in-memory `disk.raw` for `check_one_disk`.
struct Bytes(std::collections::BTreeMap<u64, Vec<u8>>);

impl Bytes {
    fn good() -> Self {
        let l = one_disk_layout(ONE_DISK_SECTOR);
        let s = ONE_DISK_SECTOR;
        let mut b = Self(Default::default());
        b.put(0, &l.mbr);
        b.put(s, &l.primary_header);
        b.put(2 * s, &l.entries);
        b.put(l.backup_entries_lba * s, &l.entries);
        b.put(l.last_lba * s, &l.backup_header);
        let mut fat = vec![0u8; 512];
        fat[82..87].copy_from_slice(b"FAT32");
        fat[510] = 0x55;
        fat[511] = 0xaa;
        b.put(l.esp.0 * s, &fat);
        b.put(l.neofs.0 * s, NEOFS_MAGIC);
        b
    }

    fn put(&mut self, at: u64, data: &[u8]) {
        for (i, &x) in data.iter().enumerate() {
            let a = at + i as u64;
            self.0.entry(a / 4096).or_insert_with(|| vec![0; 4096])[(a % 4096) as usize] = x;
        }
    }

    fn check(&self, len: u64) -> Result<(), String> {
        check_one_disk(
            len,
            &mut |at, out| {
                for (i, x) in out.iter_mut().enumerate() {
                    let a = at + i as u64;
                    *x = self
                        .0
                        .get(&(a / 4096))
                        .map_or(0, |p| p[(a % 4096) as usize]);
                }
                Ok(())
            },
            ONE_DISK_SECTOR,
        )
    }
}

#[test]
fn check_one_disk_accepts_the_layout_and_names_each_broken_rule() {
    let l = one_disk_layout(ONE_DISK_SECTOR);
    let s = ONE_DISK_SECTOR;
    Bytes::good().check(ONE_DISK_BYTES).unwrap();
    let err = Bytes::good().check(ONE_DISK_BYTES - 512).unwrap_err();
    assert!(err.contains("not 4294967296"), "{err}");
    for (at, what) in [
        (446 + 4, "protective MBR"),
        (s + 16, "primary header"),
        (2 * s + 130, "primary array"),
        (l.backup_entries_lba * s, "backup array"),
        (l.last_lba * s + 24, "backup header"),
        (l.esp.0 * s + 82, "FAT32"),
        (l.neofs.0 * s, "partition 2"),
    ] {
        let mut b = Bytes::good();
        b.put(at, &[0x5a]);
        let err = b.check(ONE_DISK_BYTES).unwrap_err();
        assert!(err.contains(what), "{what}: {err}");
    }
    // A whole-disk NeoFS superblock at one of the disk's own offsets.
    let mut b = Bytes::good();
    b.put(64 << 20, NEOFS_MAGIC);
    assert!(
        b.check(ONE_DISK_BYTES)
            .unwrap_err()
            .contains("whole-disk volume")
    );
}

#[test]
fn the_one_disk_vm_boots_its_only_disk_with_no_cdrom() {
    let args = vm_one_disk_args(
        Path::new("/fw/code.fd"),
        Path::new("/run/vars.fd"),
        Path::new("/vm/disk.raw"),
        ONE_DISK_SECTOR,
    );
    let joined = args.join(" ");
    assert!(!joined.contains("cdrom"));
    assert_eq!(joined.matches("virtio-blk-pci").count(), 1);
    assert!(joined.contains("drive=vmdisk,bootindex=0"));
    assert!(joined.contains("cache=writeback,file=/vm/disk.raw"));
}

/// OT-87 R4.3: the 4Kn one-disk image is the §4.4 oracle's S=4096 row, and
/// its virtio disk presents 4096-byte logical sectors; the 512 disk's
/// arguments don't mention a block size.
#[test]
fn the_4k_one_disk_is_the_oracle_row_and_presents_4096() {
    let l = one_disk_layout(4096);
    assert_eq!(l.last_lba, 1_048_575);
    assert_eq!(l.esp, (256, 131_327));
    assert_eq!(l.neofs, (131_328, 1_048_319));
    assert_eq!(l.neofs_total_blocks, 916_992);
    assert_eq!(
        l.neofs_total_blocks,
        one_disk_layout(ONE_DISK_SECTOR).neofs_total_blocks
    );
    let fw = (
        Path::new("/fw/code.fd"),
        Path::new("/run/vars.fd"),
        Path::new("/vm/disk.raw"),
    );
    let four = vm_one_disk_args(fw.0, fw.1, fw.2, 4096).join(" ");
    assert!(four.contains("logical_block_size=4096,physical_block_size=4096"));
    let five = vm_one_disk_args(fw.0, fw.1, fw.2, ONE_DISK_SECTOR).join(" ");
    assert!(!five.contains("logical_block_size"));
}

/// OT-87 R4.3: an install boot attaches its disks in order, each with its
/// serial; only the first is in boot order, and only a 4096-byte disk names
/// a block size.
#[test]
fn the_install_vm_boots_the_first_disk_and_names_each_one() {
    let args = vm_install_args(
        Path::new("/fw/code.fd"),
        Path::new("/run/vars.fd"),
        &[
            InstallDisk {
                path: Path::new("/vm/boot.raw"),
                serial: "LOGANSOS-BOOT",
                sector: 512,
            },
            InstallDisk {
                path: Path::new("/vm/target.raw"),
                serial: "LOGANSOS-TARGET",
                sector: 4096,
            },
        ],
    );
    let joined = args.join(" ");
    assert!(!joined.contains("cdrom"));
    assert_eq!(joined.matches("virtio-blk-pci").count(), 2);
    assert_eq!(joined.matches("bootindex").count(), 1);
    assert!(joined.contains("drive=vminst0,serial=LOGANSOS-BOOT,bootindex=0,"));
    assert!(joined.contains(
        "drive=vminst1,serial=LOGANSOS-TARGET,disable-legacy=on,disable-modern=off,logical_block_size=4096,physical_block_size=4096"
    ));
    assert_eq!(joined.matches("logical_block_size").count(), 1);
    assert!(joined.find("/vm/boot.raw") < joined.find("/vm/target.raw"));
}
