// SPDX-License-Identifier: MPL-2.0
//! OT-87 R1 (`docs/THINK-2026-10-27-ot87-installer.md` §3): LogansOS
//! installed on a VM disk. Two raw disks booted under UEFI with no `-cdrom`:
//!
//! - `boot.raw`: the ISO's bytes, unchanged. xorriso's protective MBR, GPT and
//!   EFI system partition let OVMF boot `\EFI\BOOT\BOOTX64.EFI` (Limine) from
//!   a plain disk.
//! - `data.raw`: a whole-disk NeoFS volume (`neofs-mkfs`, variant
//!   `arthur-memory-system`: Arthur's memory directory and the empty
//!   `.system/` sign-in keeps its record in, OT-66 L1). The one-disk
//!   layout's NeoFS partition is the same. The kernel mounts the first device whose NeoFS probe
//!   passes (`kernel/src/fs/mod.rs`), so the boot disk must carry no NeoFS
//!   superblock: [`check_vm_layout`] holds both disks to that.
//!
//! OT-87 R3 adds the one-disk layout (`vm-image --one-disk`): `disk.raw`, a
//! fixed 4 GiB GPT disk laid out by `gpt_logic::gpt_layout` (THINK §4.4),
//! entry 1 the ESP (the ISO's tree on FAT32), entry 2 a LogansOS NeoFS
//! partition. Its GPT bytes are the §4.4 oracle's, GUIDs included.
//!
//! The firmware is the edk2 build QEMU v10.2.3 ships (`pc-bios/`,
//! edk2-stable202408.01), which the dev container's QEMU install unpacks;
//! [`find_ovmf`] holds it to its BLAKE3 pins.

use std::path::{Path, PathBuf};

/// Where `cargo xtask vm-image` writes the two disks.
pub const VM_DIR_REL: &str = "target/vm";
pub const BOOT_DISK: &str = "boot.raw";
pub const DATA_DISK: &str = "data.raw";
/// The data disk's size: `neofs-mkfs`'s default (three superblock copies,
/// the last at 64 MiB).
pub const DATA_DISK_MIB: u64 = 128;

/// OT-87 R3: the one-disk image.
pub const ONE_DISK: &str = "disk.raw";
/// Fixed size (THINK §9, decision 2): the §4.4 oracle's 4 GiB disk.
pub const ONE_DISK_BYTES: u64 = 4 << 30;
pub const ONE_DISK_SECTOR: u64 = 512;
/// The oracle's GUIDs (`tools/block-guard-logic-tests/fixtures/gpt_oracle.py`),
/// as GPT stores them: 4c6f6761-6e4f-5321-8000-000000000001 / …e5 / …f5.
pub const ONE_DISK_GUID: [u8; 16] = [
    0x61, 0x67, 0x6f, 0x4c, 0x4f, 0x6e, 0x21, 0x53, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
];
pub const ONE_DISK_ESP_GUID: [u8; 16] = [
    0x61, 0x67, 0x6f, 0x4c, 0x4f, 0x6e, 0x21, 0x53, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xe5,
];
pub const ONE_DISK_NEOFS_GUID: [u8; 16] = [
    0x61, 0x67, 0x6f, 0x4c, 0x4f, 0x6e, 0x21, 0x53, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xf5,
];

/// The one-disk image's table at logical sector size `s`: [`ONE_DISK_SECTOR`],
/// or 4096 (OT-87 R4.3, the §4.4 oracle's second geometry).
pub fn one_disk_layout(s: u64) -> crate::gpt_logic::Layout {
    crate::gpt_logic::gpt_layout(
        s,
        ONE_DISK_BYTES / s - 1,
        ONE_DISK_GUID,
        ONE_DISK_ESP_GUID,
        ONE_DISK_NEOFS_GUID,
        false,
    )
    .expect("4 GiB holds the ESP and a NeoFS partition")
}

/// Fills a buffer from a byte offset of the disk being checked.
pub type ReadAt<'a> = dyn FnMut(u64, &mut [u8]) -> Result<(), String> + 'a;

/// What `disk.raw` must be: exactly [`ONE_DISK_BYTES`], the layout's MBR,
/// headers and both arrays byte for byte, a FAT32 boot sector at the ESP, a
/// NeoFS superblock at partition 2, and none at the disk's own superblock
/// offsets (so the kernel's whole-device probe and the guard's whole-disk
/// NeoFS rule see no volume there). `read` fills a buffer from a byte offset.
/// Laid out for logical sector size `s`.
pub fn check_one_disk(len: u64, read: &mut ReadAt, s: u64) -> Result<(), String> {
    if len != ONE_DISK_BYTES {
        return Err(format!("{ONE_DISK} is {len} bytes, not {ONE_DISK_BYTES}"));
    }
    let layout = one_disk_layout(s);
    let mut expect = |what: &str, at: u64, want: &[u8]| -> Result<(), String> {
        let mut got = vec![0u8; want.len()];
        read(at, &mut got)?;
        if got != want {
            return Err(format!(
                "{ONE_DISK}: {what} at {at:#x} is not the R3 layout's"
            ));
        }
        Ok(())
    };
    expect("the protective MBR", 0, &layout.mbr)?;
    expect("the primary header", s, &layout.primary_header)?;
    expect("the primary array", 2 * s, &layout.entries)?;
    expect(
        "the backup array",
        layout.backup_entries_lba * s,
        &layout.entries,
    )?;
    expect(
        "the backup header",
        layout.last_lba * s,
        &layout.backup_header,
    )?;
    let mut boot = [0u8; 512];
    read(layout.esp.0 * s, &mut boot)?;
    if &boot[82..87] != b"FAT32" || boot[510..512] != [0x55, 0xaa] {
        return Err(format!("{ONE_DISK}: the ESP holds no FAT32 boot sector"));
    }
    if !has_neofs_at(read, layout.neofs.0 * s)? {
        return Err(format!("{ONE_DISK}: partition 2 holds no NeoFS superblock"));
    }
    for offset in NEOFS_SUPERBLOCK_OFFSETS {
        if has_neofs_at(read, offset)? {
            return Err(format!(
                "{ONE_DISK} has a NeoFS superblock at {offset:#x}: a whole-disk volume"
            ));
        }
    }
    Ok(())
}

fn has_neofs_at(read: &mut ReadAt, at: u64) -> Result<bool, String> {
    let mut magic = [0u8; 8];
    read(at, &mut magic)?;
    Ok(magic == *NEOFS_MAGIC)
}

/// NeoFS v1's superblock magic and its three copies: the kernel write
/// guard's, the single source the kernel's NeoFS format checks against.
use crate::write_guard_logic::{NEOFS_MAGIC, NEOFS_SUPERBLOCK_OFFSETS};

/// The EFI system partition type GUID, C12A7328-F81F-11D2-BA4B-00A0C93EC93B
/// (UEFI spec 2.10 §5.3.3), as GPT stores it: the first three fields
/// little-endian, the last two as written.
pub const ESP_TYPE_GUID: [u8; 16] = [
    0x28, 0x73, 0x2a, 0xc1, 0x1f, 0xf8, 0xd2, 0x11, 0xba, 0x4b, 0x00, 0xa0, 0xc9, 0x3e, 0xc9, 0x3b,
];

/// The pinned firmware: QEMU v10.2.3's `pc-bios/edk2-x86_64-code.fd` and
/// `edk2-i386-vars.fd` (edk2-stable202408.01, commit 4dfdca63), as the dev
/// container's `make install` puts them in `share/qemu`. The Dockerfile
/// checks the same files by SHA-256.
pub const OVMF_CODE: &str = "edk2-x86_64-code.fd";
pub const OVMF_VARS: &str = "edk2-i386-vars.fd";
pub const OVMF_CODE_BLAKE3: &str =
    "e72f887c74e6bde078b14c149daeaf656cf6b19d7626ca2a7ddf012561c32909";
pub const OVMF_VARS_BLAKE3: &str =
    "bc7ba590df577bc4b7591cb3ea6861ccbd86b77d3734e473e7b23d805b7cc1b8";
/// A directory holding [`OVMF_CODE`] and [`OVMF_VARS`] (e.g. a Windows QEMU
/// install's `share`); checked first.
pub const OVMF_DIR_ENV: &str = "NEO_OVMF_DIR";
/// Where QEMU installs its blobs: the dev container's from-source build, then
/// a distribution's.
pub const OVMF_DEFAULT_DIRS: [&str; 2] = ["/usr/local/share/qemu", "/usr/share/qemu"];

/// What `boot.raw` and `data.raw` must be. `iso` is the ISO they were made
/// from. Errors name the first rule broken.
pub fn check_vm_layout(boot: &[u8], iso: &[u8], data: &[u8]) -> Result<(), String> {
    if boot != iso {
        return Err(format!(
            "{BOOT_DISK} is not the ISO's bytes ({} bytes against {})",
            boot.len(),
            iso.len()
        ));
    }
    if boot.is_empty() || !boot.len().is_multiple_of(512) {
        return Err(format!(
            "{BOOT_DISK} is {} bytes, not a whole number of 512-byte sectors",
            boot.len()
        ));
    }
    check_boot_disk_partitions(boot)?;
    for offset in NEOFS_SUPERBLOCK_OFFSETS {
        if has_neofs_magic(boot, offset) {
            return Err(format!(
                "{BOOT_DISK} has a NeoFS superblock at {offset:#x}: the kernel would mount the boot disk"
            ));
        }
    }
    let want = DATA_DISK_MIB * 1024 * 1024;
    if data.len() as u64 != want {
        return Err(format!(
            "{DATA_DISK} is {} bytes, not {want} ({DATA_DISK_MIB} MiB)",
            data.len()
        ));
    }
    for offset in NEOFS_SUPERBLOCK_OFFSETS {
        if !has_neofs_magic(data, offset) {
            return Err(format!(
                "{DATA_DISK} has no NeoFS superblock at {offset:#x}"
            ));
        }
    }
    Ok(())
}

fn has_neofs_magic(disk: &[u8], offset: u64) -> bool {
    let start = offset as usize;
    disk.get(start..start + NEOFS_MAGIC.len()) == Some(&NEOFS_MAGIC[..])
}

/// UEFI boots the disk from its GPT: a protective MBR (one type-0xEE entry),
/// the `EFI PART` header at LBA 1, and an EFI system partition in the entry
/// array the header points at.
fn check_boot_disk_partitions(boot: &[u8]) -> Result<(), String> {
    if boot.get(510..512) != Some(&[0x55, 0xaa][..]) {
        return Err(format!("{BOOT_DISK} has no MBR boot signature at 510"));
    }
    if boot[446 + 4] != 0xee {
        return Err(format!(
            "{BOOT_DISK}'s first MBR entry is type {:#04x}, not the protective 0xee",
            boot[446 + 4]
        ));
    }
    if boot.get(512..520) != Some(&b"EFI PART"[..]) {
        return Err(format!("{BOOT_DISK} has no GPT header at LBA 1"));
    }
    let header = &boot[512..];
    let entries_lba = u64::from_le_bytes(header[72..80].try_into().unwrap());
    let count = u32::from_le_bytes(header[80..84].try_into().unwrap()) as u64;
    let size = u32::from_le_bytes(header[84..88].try_into().unwrap()) as u64;
    if size < 128 || count == 0 {
        return Err(format!(
            "{BOOT_DISK}'s GPT header lists {count} entries of {size} bytes"
        ));
    }
    let has_esp = (0..count).any(|i| {
        let start = (entries_lba * 512 + i * size) as usize;
        boot.get(start..start + 16) == Some(&ESP_TYPE_GUID[..])
    });
    if !has_esp {
        return Err(format!("{BOOT_DISK}'s GPT has no EFI system partition"));
    }
    Ok(())
}

/// The firmware pair a VM boots with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ovmf {
    pub code: PathBuf,
    pub vars: PathBuf,
}

/// Where to look for the firmware: `$NEO_OVMF_DIR`, then
/// [`OVMF_DEFAULT_DIRS`].
pub fn ovmf_search_dirs(env_dir: Option<&Path>) -> Vec<PathBuf> {
    env_dir
        .map(Path::to_path_buf)
        .into_iter()
        .chain(OVMF_DEFAULT_DIRS.iter().map(PathBuf::from))
        .collect()
}

/// The first of `dirs` holding both files. `digest` hashes a file; with
/// `require_pin` both must match their pins (the gate's scenario), else a
/// mismatch comes back as a warning (`vm-run` on a host's own QEMU). The
/// error says where it looked.
pub fn find_ovmf(
    dirs: &[PathBuf],
    require_pin: bool,
    digest: impl Fn(&Path) -> Result<String, String>,
) -> Result<(Ovmf, Option<String>), String> {
    for dir in dirs {
        let ovmf = Ovmf {
            code: dir.join(OVMF_CODE),
            vars: dir.join(OVMF_VARS),
        };
        if !ovmf.code.is_file() || !ovmf.vars.is_file() {
            continue;
        }
        let mut mismatches = Vec::new();
        for (path, pin) in [
            (&ovmf.code, OVMF_CODE_BLAKE3),
            (&ovmf.vars, OVMF_VARS_BLAKE3),
        ] {
            let got = digest(path)?;
            if got != pin {
                mismatches.push(format!("{} is blake3 {got}, pinned {pin}", path.display()));
            }
        }
        if mismatches.is_empty() {
            return Ok((ovmf, None));
        }
        let note = mismatches.join("; ");
        if require_pin {
            return Err(format!(
                "INFRA: the OVMF in {} is not the pinned build (QEMU v10.2.3's edk2-stable202408.01): {note}",
                dir.display()
            ));
        }
        return Ok((ovmf, Some(note)));
    }
    Err(format!(
        "INFRA: no OVMF found ({OVMF_CODE} + {OVMF_VARS}) in {}; run inside the dev container, or set {OVMF_DIR_ENV}",
        dirs.iter()
            .map(|d| d.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

/// The QEMU arguments for the installed layout: the firmware (code read-only,
/// `vars` a per-run copy), the boot disk first in boot order, then the data
/// disk. No `-cdrom`.
pub fn vm_disk_args(ovmf_code: &Path, vars: &Path, boot: &Path, data: &Path) -> Vec<String> {
    vec![
        "-drive".into(),
        format!(
            "if=pflash,format=raw,unit=0,readonly=on,file={}",
            ovmf_code.display()
        ),
        "-drive".into(),
        format!("if=pflash,format=raw,unit=1,file={}", vars.display()),
        "-drive".into(),
        format!("if=none,id=vmboot,format=raw,file={}", boot.display()),
        "-device".into(),
        "virtio-blk-pci,drive=vmboot,bootindex=0,disable-legacy=on,disable-modern=off".into(),
        "-drive".into(),
        format!(
            "if=none,id=vmdata,format=raw,cache=writeback,file={}",
            data.display()
        ),
        "-device".into(),
        "virtio-blk-pci,drive=vmdata,disable-legacy=on,disable-modern=off".into(),
    ]
}

/// The QEMU arguments for the one-disk layout: the firmware, then `disk` as
/// the only disk. No `-cdrom`.
/// For a disk of logical sector size `s`: above 512 the virtio disk presents
/// it (`logical_block_size`), so the firmware and the kernel see a 4Kn disk.
pub fn vm_one_disk_args(ovmf_code: &Path, vars: &Path, disk: &Path, s: u64) -> Vec<String> {
    let block = if s == 512 {
        String::new()
    } else {
        format!(",logical_block_size={s},physical_block_size={s}")
    };
    vec![
        "-drive".into(),
        format!(
            "if=pflash,format=raw,unit=0,readonly=on,file={}",
            ovmf_code.display()
        ),
        "-drive".into(),
        format!("if=pflash,format=raw,unit=1,file={}", vars.display()),
        "-drive".into(),
        format!(
            "if=none,id=vmdisk,format=raw,cache=writeback,file={}",
            disk.display()
        ),
        "-device".into(),
        format!(
            "virtio-blk-pci,drive=vmdisk,bootindex=0,disable-legacy=on,disable-modern=off{block}"
        ),
    ]
}

/// One disk of an install boot: its image, its virtio serial (how the
/// kernel's install scenario tells the disks apart) and its logical sector
/// size.
pub struct InstallDisk<'a> {
    pub path: &'a Path,
    pub serial: &'a str,
    pub sector: u64,
}

/// The QEMU arguments for an install boot (OT-87 R4.3): the firmware, then
/// `disks` in order, the first in boot order. No `-cdrom`.
pub fn vm_install_args(ovmf_code: &Path, vars: &Path, disks: &[InstallDisk]) -> Vec<String> {
    let mut args = vec![
        "-drive".into(),
        format!(
            "if=pflash,format=raw,unit=0,readonly=on,file={}",
            ovmf_code.display()
        ),
        "-drive".into(),
        format!("if=pflash,format=raw,unit=1,file={}", vars.display()),
    ];
    for (i, disk) in disks.iter().enumerate() {
        let block = if disk.sector == 512 {
            String::new()
        } else {
            format!(
                ",logical_block_size={0},physical_block_size={0}",
                disk.sector
            )
        };
        let boot = if i == 0 { ",bootindex=0" } else { "" };
        args.push("-drive".into());
        args.push(format!(
            "if=none,id=vminst{i},format=raw,cache=writeback,file={}",
            disk.path.display()
        ));
        args.push("-device".into());
        args.push(format!(
            "virtio-blk-pci,drive=vminst{i},serial={}{boot},disable-legacy=on,disable-modern=off{block}",
            disk.serial
        ));
    }
    args
}

#[cfg(test)]
#[path = "vm_image_tests.rs"]
mod tests;
