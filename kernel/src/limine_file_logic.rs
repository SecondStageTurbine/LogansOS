// SPDX-License-Identifier: MPL-2.0
//! The fields of Limine's `struct limine_file` the kernel reads past the
//! `limine` crate, by their offsets in Limine's protocol (`PROTOCOL.md`,
//! Limine v8.6.0, `7cddb611`):
//!
//! | Offset | Size | Field |
//! |---|---|---|
//! | 0 | 8 | revision |
//! | 8 | 8 | address |
//! | 16 | 8 | size |
//! | 24 | 8 | path |
//! | 32 | 8 | cmdline |
//! | 40 | 4 | media_type |
//! | 44 | 4 | unused |
//! | 48 | 4 | tftp_ip |
//! | 52 | 4 | tftp_port |
//! | 56 | 4 | partition_index |
//! | 60 | 4 | mbr_disk_id |
//! | 64 | 16 | gpt_disk_uuid |
//! | 80 | 16 | gpt_part_uuid |
//! | 96 | 16 | part_uuid |
//!
//! Why not the crate's field: `limine` 0.6.3's `File` omits `unused` and
//! `tftp_port`, so its `gpt_disk_uuid` sits at 56, and its `Uuid` isn't
//! `repr(C)`. Read through the crate, the boot medium's GUID came back as
//! its first eight bytes and eight zeros, which never equals a disk's GUID,
//! so the install claim's boot-disk refusal (THINK §2, layer 3) never fired.
//! `address`, `size`, `path` and `cmdline` precede the gap; the crate reads
//! them right.

/// The struct's size: every field through `part_uuid`.
pub const FILE_BYTES: usize = 112;
pub const GPT_DISK_UUID_OFFSET: usize = 64;
/// The entry's command line (`const char *`): Limine v8.6.0 fills it on the
/// executable file's response, and answers no separate command-line request
/// (OT-100 R3: the first CI run saw `persist=off` on a `persist=on` entry).
pub const CMDLINE_OFFSET: usize = 32;

/// The command-line pointer Limine stored in the file, as an address (zero
/// when it gave none).
#[allow(dead_code)] // the installer build reads only the GUID
pub fn cmdline_ptr(file: &[u8; FILE_BYTES]) -> u64 {
    let mut raw = [0u8; 8];
    raw.copy_from_slice(&file[CMDLINE_OFFSET..CMDLINE_OFFSET + 8]);
    u64::from_le_bytes(raw)
}

/// The GPT disk GUID of the medium the file was loaded from, as GPT stores
/// it (Limine copies the header's 16 bytes). `None` when Limine reports
/// zero: the medium has no GPT.
pub fn gpt_disk_guid(file: &[u8; FILE_BYTES]) -> Option<[u8; 16]> {
    let mut out = [0u8; 16];
    out.copy_from_slice(&file[GPT_DISK_UUID_OFFSET..GPT_DISK_UUID_OFFSET + 16]);
    (out != [0; 16]).then_some(out)
}
