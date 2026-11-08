// SPDX-License-Identifier: MPL-2.0
//! Byte oracle for the OT-61 C2.1 `SYS_HANDLE_INFO` record (ADR-028's rule:
//! hand-laid bytes, not a round trip). The kernel encoder
//! (`kernel/src/sched/handle_info_logic.rs`) and libneo's decoder
//! (`userland/libneo/src/handle_info.rs`) are each checked against the same
//! literal byte arrays, written out by hand from the layout table.

#![allow(dead_code)]

#[path = "../../../kernel/src/sched/handle_info_logic.rs"]
mod kernel;

#[path = "../../../userland/libneo/src/handle_info.rs"]
mod libneo;

#[cfg(test)]
mod tests {
    use super::{kernel, libneo};

    /// A read+write `File` handle (FileRights 0b11) with `file-caps` meta 0x05
    /// and semantic 0x03.
    const FILE_RW: [u8; 16] = [
        0x01, 0x00, 0x00, 0x00, // version 1
        0x03, 0x00, 0x00, 0x00, // kind 3 = File
        0x03, 0x00, 0x00, 0x00, // file_rights READ|WRITE
        0x05, // cap_meta
        0x03, // cap_semantic
        0x00, // flags
        0x00, // reserved
    ];

    /// A `BrowserRuntime` capability: kind 15, no file rights, meta 0x81,
    /// semantic 0x01.
    const BROWSER_RUNTIME: [u8; 16] = [
        0x01, 0x00, 0x00, 0x00, // version 1
        0x0F, 0x00, 0x00, 0x00, // kind 15
        0x00, 0x00, 0x00, 0x00, // file_rights 0
        0x81, // cap_meta
        0x01, // cap_semantic
        0x00, // flags
        0x00, // reserved
    ];

    /// A RootDirCap (OT-66 K1a): kind 3 = File, FileRights
    /// READ|WRITE|LOOKUP|READDIR|EXECUTE|MOUNT|GRANT|DERIVE = 0x0B00_001F,
    /// meta USE|DERIVE|GRANT|REVOKE = 0x0F, semantic 0x5F (MOUNT relocated to
    /// bit 6), flags ROOT_DIR.
    const ROOT_DIR_CAP: [u8; 16] = [
        0x01, 0x00, 0x00, 0x00, // version 1
        0x03, 0x00, 0x00, 0x00, // kind 3 = File
        0x1F, 0x00, 0x00, 0x0B, // file_rights 0x0B00_001F
        0x0F, // cap_meta
        0x5F, // cap_semantic
        0x01, // flags: ROOT_DIR
        0x00, // reserved
    ];

    #[test]
    fn kernel_encoder_matches_the_hand_laid_records() {
        assert_eq!(
            kernel::encode(kernel::HandleInfo {
                kind: kernel::KIND_FILE,
                file_rights: 0b11,
                cap_meta: 0x05,
                cap_semantic: 0x03,
                flags: 0,
            }),
            FILE_RW
        );
        assert_eq!(
            kernel::encode(kernel::HandleInfo {
                kind: kernel::KIND_FILE,
                file_rights: 0x0B00_001F,
                cap_meta: 0x0F,
                cap_semantic: 0x5F,
                flags: kernel::FLAG_ROOT_DIR,
            }),
            ROOT_DIR_CAP
        );
        assert_eq!(
            kernel::encode(kernel::HandleInfo {
                kind: kernel::KIND_BROWSER_RUNTIME,
                file_rights: 0,
                cap_meta: 0x81,
                cap_semantic: 0x01,
                flags: 0,
            }),
            BROWSER_RUNTIME
        );
        assert_eq!(kernel::RECORD_LEN, 16);
    }

    #[test]
    fn libneo_decoder_reads_the_hand_laid_records() {
        let f = libneo::HandleInfo::decode(&FILE_RW).expect("v1 record");
        assert_eq!(f.kind, libneo::HandleKind::File);
        assert_eq!(
            (f.file_rights, f.cap_meta, f.cap_semantic),
            (0b11, 0x05, 0x03)
        );
        assert_eq!(f.flags, 0);
        let b = libneo::HandleInfo::decode(&BROWSER_RUNTIME).expect("v1 record");
        assert_eq!(b.kind, libneo::HandleKind::BrowserRuntime);
        let r = libneo::HandleInfo::decode(&ROOT_DIR_CAP).expect("v1 record");
        assert_eq!(r.kind, libneo::HandleKind::File);
        assert_eq!(r.file_rights, 0x0B00_001F);
        assert_eq!((r.cap_meta, r.cap_semantic), (0x0F, 0x5F));
        assert_eq!(r.flags & libneo::FLAG_ROOT_DIR, libneo::FLAG_ROOT_DIR);
        assert_eq!(libneo::RECORD_LEN, 16);
    }

    #[test]
    fn libneo_decoder_refuses_other_versions_and_unknown_kinds() {
        let mut v2 = FILE_RW;
        v2[0] = 2;
        assert_eq!(libneo::HandleInfo::decode(&v2), None);
        let mut unknown = FILE_RW;
        unknown[4] = 0x63;
        let d = libneo::HandleInfo::decode(&unknown).expect("v1");
        assert_eq!(d.kind, libneo::HandleKind::Unknown(0x63));
    }

    /// The kind codes are ABI: both sides must agree on every one.
    #[test]
    fn kind_codes_agree() {
        for (code, kind) in [
            (kernel::KIND_CHANNEL, libneo::HandleKind::Channel),
            (kernel::KIND_PROCESS, libneo::HandleKind::Process),
            (kernel::KIND_FILE, libneo::HandleKind::File),
            (kernel::KIND_SOCKET, libneo::HandleKind::Socket),
            (kernel::KIND_TCP_SOCKET, libneo::HandleKind::TcpSocket),
            (kernel::KIND_TCP_LISTENER, libneo::HandleKind::TcpListener),
            (kernel::KIND_NET_DIAL, libneo::HandleKind::NetDial),
            (kernel::KIND_TENSOR, libneo::HandleKind::Tensor),
            (kernel::KIND_MODEL, libneo::HandleKind::Model),
            (kernel::KIND_BUFFER, libneo::HandleKind::Buffer),
            (kernel::KIND_INPUT_FOCUS, libneo::HandleKind::InputFocus),
            (kernel::KIND_GPU_SUBMIT, libneo::HandleKind::GpuSubmit),
            (kernel::KIND_INPUT_SYSTEM, libneo::HandleKind::InputSystem),
            (kernel::KIND_GPU_COMPUTE, libneo::HandleKind::GpuCompute),
            (
                kernel::KIND_BROWSER_RUNTIME,
                libneo::HandleKind::BrowserRuntime,
            ),
            (kernel::KIND_INSTALLER, libneo::HandleKind::Installer),
            (kernel::KIND_SYSTEM_STORE, libneo::HandleKind::SystemStore),
            (
                kernel::KIND_MOUNT_AUTHORITY,
                libneo::HandleKind::MountAuthority,
            ),
            (kernel::KIND_POWER_CONTROL, libneo::HandleKind::PowerControl),
        ] {
            assert_eq!(libneo::HandleKind::from_code(code), kind, "code {code}");
        }
    }
}
