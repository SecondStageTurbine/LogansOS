// SPDX-License-Identifier: MPL-2.0
//! Byte oracle for the boot medium's GUID in Limine's `struct limine_file`
//! (`kernel/src/limine_file_logic.rs`). The fixture is the response Limine
//! v8.6.0 gave the kernel in `install_console` (bytes 40..112 dumped from
//! the kernel; 0..40 are the pointers and sizes, zero here), checked against
//! the protocol's offsets; then the `limine` crate's divergence, pinned.

#![allow(dead_code)]

#[path = "../../../kernel/src/limine_file_logic.rs"]
mod logic;

#[cfg(test)]
mod tests {
    use super::logic;

    /// The disk GUID GPT stores on that boot medium, as the kernel's own GPT
    /// read gave it (`install: list slot=0 ... disk_guid=`).
    const DISK_GUID: [u8; 16] = [
        0x36, 0x0e, 0x4a, 0xc8, 0x53, 0xf4, 0xea, 0x45, 0xba, 0xa9, 0xdd, 0x9a, 0x18, 0x22, 0x4f,
        0xb6,
    ];

    /// `struct limine_file` as Limine filled it.
    fn fixture() -> [u8; logic::FILE_BYTES] {
        let mut f = [0u8; logic::FILE_BYTES];
        let tail: [u8; 72] = [
            0x00, 0x00, 0x00, 0x00, // 40 media_type: generic
            0x00, 0x00, 0x00, 0x00, // 44 unused
            0x00, 0x00, 0x00, 0x00, // 48 tftp_ip
            0x00, 0x00, 0x00, 0x00, // 52 tftp_port
            0x00, 0x00, 0x00, 0x00, // 56 partition_index
            0x00, 0x00, 0x00, 0x00, // 60 mbr_disk_id
            // 64 gpt_disk_uuid
            0x36, 0x0e, 0x4a, 0xc8, 0x53, 0xf4, 0xea, 0x45, 0xba, 0xa9, 0xdd, 0x9a, 0x18, 0x22,
            0x4f, 0xb6, //
            // 80 gpt_part_uuid
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, //
            // 96 part_uuid
            0x36, 0x0e, 0x4a, 0xc8, 0x53, 0xf4, 0xea, 0x45, 0xba, 0xa9, 0xdd, 0x9a, 0x18, 0x22,
            0x4f, 0xb6,
        ];
        f[40..].copy_from_slice(&tail);
        f
    }

    #[test]
    fn the_disk_guid_is_read_at_the_protocol_offset() {
        assert_eq!(logic::gpt_disk_guid(&fixture()), Some(DISK_GUID));
    }

    #[test]
    fn a_medium_without_gpt_has_no_guid() {
        let mut f = fixture();
        f[64..80].fill(0);
        assert_eq!(logic::gpt_disk_guid(&f), None);
    }

    #[test]
    fn neighbouring_fields_do_not_leak_in() {
        let mut f = fixture();
        f[56..64].fill(0xff); // partition_index, mbr_disk_id
        f[80..96].fill(0xee); // gpt_part_uuid
        assert_eq!(logic::gpt_disk_guid(&f), Some(DISK_GUID));
    }

    /// The command line is the pointer at offset 32, little-endian, between
    /// `path` and `media_type`.
    #[test]
    fn the_cmdline_pointer_is_read_at_the_protocol_offset() {
        let mut f = fixture();
        f[24..32].fill(0xaa); // path
        f[32..40].copy_from_slice(&0xffff_8000_1234_5678u64.to_le_bytes());
        assert_eq!(logic::cmdline_ptr(&f), 0xffff_8000_1234_5678);
        assert_eq!(logic::cmdline_ptr(&fixture()), 0);
    }

    /// The divergence `limine_file_logic` exists for: the crate puts the
    /// field 8 bytes early and doesn't lay `Uuid` out as C does. If a crate
    /// update fixes either, this fails, and the raw read can be revisited.
    #[test]
    fn the_limine_crate_misplaces_the_field() {
        assert_eq!(
            core::mem::offset_of!(limine::file::File, gpt_disk_uuid),
            logic::GPT_DISK_UUID_OFFSET - 8
        );
        assert_ne!(core::mem::offset_of!(limine::uuid::Uuid, a), 0);
    }
}
