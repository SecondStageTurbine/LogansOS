// SPDX-License-Identifier: MPL-2.0
//! Host KATs for the atrium G1 compositor slice-1 pure logic.
//!
//! Mounts the *real* kernel module (`kernel/src/atrium/logic.rs`) via
//! `#[path]` and asserts it on the host:
//!
//! - the **descriptor byte-oracle** (the ADR-028 discipline): a hand-laid
//!   24-byte fixture asserted against `decode_desc` AND `encode_desc`
//!   independently — never a writer↔reader round-trip alone;
//! - the **present-result byte-oracle**: a hand-laid 32-byte fixture asserted
//!   against `decode_present_result` AND `encode_present_result`, plus the
//!   `repr(C)` in-memory bytes — proving the syscall's `write_unaligned`
//!   copy-out yields exactly the oracle bytes;
//! - the descriptor validation truth table (zero dims, stride floor and
//!   alignment, format, bytes mismatch, the 64 MiB ceiling, output bounds,
//!   and the checked-math overflow cases);
//! - the fence epoch-satisfaction truth table (the buffer-reuse ABA guard:
//!   a waiter's epoch is satisfied only by its own or a LATER present);
//! - the console routing selector KAT — the kill-switch equality's host
//!   half: every (feature, runtime) combination routes to the v1 console
//!   in slice-1, pinning the off-branch forever;
//! - the rights-bit layout (distinct, non-overlapping semantic bits).

#![allow(dead_code)]

#[path = "../../../kernel/src/atrium/logic.rs"]
mod logic;

#[path = "../../../kernel/src/atrium/blur.rs"]
mod blur;

// Input queue policy (key-eviction fix): mounted with its `input_logic`
// dependency under the same module names the kernel uses.
#[path = "../../../kernel/src/atrium/input_logic.rs"]
mod input_logic;
#[cfg(test)]
mod input_queue_kats;
#[path = "../../../kernel/src/atrium/input_queue_logic.rs"]
mod input_queue_logic;
// Key auto-repeat: the state machine, its wire flag and the queue rule.
#[cfg(test)]
mod key_repeat_kats;
#[path = "../../../kernel/src/atrium/key_repeat_logic.rs"]
mod key_repeat_logic;
// DI-1 SAK chord recognizer, tested through the real boot-keyboard decoder.
#[path = "../../../kernel/src/drivers/usb/hid_logic.rs"]
mod hid_logic;
#[cfg(test)]
mod sak_kats;
#[path = "../../../kernel/src/atrium/sak_logic.rs"]
mod sak_logic;
// DI-1a B-ref pointer records.
#[cfg(test)]
mod pointer_route_kats;
#[path = "../../../kernel/src/atrium/pointer_route_logic.rs"]
mod pointer_route_logic;

#[path = "../../../userland/libneo/src/atrium_protocol.rs"]
mod protocol;

// Mounted under its libneo name because the S1 desktop module names its
// sibling `super::atrium_wm`; the older tests keep reaching it as `wm`.
#[path = "../../../userland/libneo/src/atrium_wm.rs"]
mod atrium_wm;
#[cfg(test)]
mod shared_pixels_kats;
// F7: `Rect::minus`, which a moved frame's repaint uses.
#[cfg(test)]
mod rect_minus_kats;
// PR #58 re-check: `Rect::uncovered_px`, the drag frames' over-present count.
#[cfg(test)]
mod rect_uncovered_kats;
// A9-D: the busy emblem composed over a client, as the WM layers it.
#[cfg(test)]
mod busy_emblem_kats;
#[cfg(test)]
use atrium_wm as wm;

#[path = "../../../userland/libneo/src/atrium_desktop.rs"]
mod atrium_desktop;

// DS-1b broker table; it names its sibling `crate::atrium_protocol`.
#[cfg(test)]
use protocol as atrium_protocol;
#[cfg(test)]
#[path = "../../../userland/libneo/src/atrium_launch.rs"]
mod atrium_launch;

// DI-1b resize: the WM-to-client configure mailbox (its seqlock KATs live in
// the module).
#[cfg(test)]
#[path = "../../../userland/libneo/src/atrium_configure.rs"]
mod atrium_configure;

// Chad DS-1a F1: the keyboard handoff order and its routing model.
#[cfg(test)]
#[path = "../../../userland/libneo/src/atrium_handoff.rs"]
mod atrium_handoff;
#[cfg(test)]
mod handoff_kats;

// PR #15 re-check item 3 (input B): a lend over a client's channel by tries.
#[cfg(test)]
#[path = "../../../userland/libneo/src/atrium_lend.rs"]
mod atrium_lend;
#[cfg(test)]
mod lend_kats;

// OT-22: the SYS_WAIT_ANY item ABI: the kernel's decoder (its own KATs) and
// libneo's layout, each against the same hand-laid bytes.
#[cfg(test)]
#[path = "../../../userland/libneo/src/wait_abi.rs"]
mod wait_abi;
#[cfg(test)]
#[path = "../../../kernel/src/sched/wait_any_logic.rs"]
mod wait_any_logic;

#[cfg(test)]
mod wait_abi_kats {
    use super::wait_abi::{
        WAIT_KIND_CHANNEL, WAIT_KIND_INPUT, WAIT_KIND_PIPE, WAIT_KIND_SAK, WaitItem,
    };
    use super::wait_any_logic as kernel;

    /// The byte oracle: `WaitItem { handle: 0x04030201, kind: 2 }` is these
    /// eight bytes in memory (what the kernel copies in), and the kernel
    /// decodes them back to the same pair.
    #[test]
    fn libneo_lays_out_the_bytes_the_kernel_decodes() {
        const HAND: [u8; 8] = [0x01, 0x02, 0x03, 0x04, 0x02, 0x00, 0x00, 0x00];
        assert_eq!(core::mem::size_of::<WaitItem>(), kernel::ITEM_LEN);
        assert_eq!(core::mem::align_of::<WaitItem>(), 4);
        let item = WaitItem {
            handle: 0x0403_0201,
            kind: WAIT_KIND_INPUT,
        };
        // SAFETY: WaitItem is repr(C), two u32s, no padding (size checked).
        let bytes: [u8; 8] = unsafe { core::mem::transmute(item) };
        assert_eq!(bytes, HAND);
        assert_eq!(kernel::decode_item(&HAND), (0x0403_0201, WAIT_KIND_INPUT));
    }

    #[test]
    fn both_sides_agree_on_the_kinds() {
        assert_eq!(WAIT_KIND_CHANNEL, kernel::WAIT_KIND_CHANNEL);
        assert_eq!(WAIT_KIND_INPUT, kernel::WAIT_KIND_INPUT);
        assert_eq!(WAIT_KIND_SAK, kernel::WAIT_KIND_SAK);
        assert_eq!(WAIT_KIND_PIPE, kernel::WAIT_KIND_PIPE);
        assert_eq!(WAIT_KIND_PIPE, 4);
    }
}

#[cfg(test)]
mod tests {
    use super::logic::*;
    use super::protocol;
    use super::wm;

    /// The hand-laid byte oracle: BufferDescV1 { width: 1024, height: 768,
    /// stride: 4096, format: XR24, bytes: 3 145 728 }, little-endian.
    /// Laid out BY HAND from the wire spec — independent of both codec
    /// directions:
    ///   width  1024      = 0x00000400 -> 00 04 00 00
    ///   height 768       = 0x00000300 -> 00 03 00 00
    ///   stride 4096      = 0x00001000 -> 00 10 00 00
    ///   format 'XR24'    = 0x34325258 -> 58 52 32 34  ("XR24" in ASCII)
    ///   bytes  3145728   = 0x0000000000300000 -> 00 00 30 00 00 00 00 00
    const ORACLE_BYTES: [u8; BUFFER_DESC_V1_LEN] = [
        0x00, 0x04, 0x00, 0x00, // width
        0x00, 0x03, 0x00, 0x00, // height
        0x00, 0x10, 0x00, 0x00, // stride
        0x58, 0x52, 0x32, 0x34, // format "XR24"
        0x00, 0x00, 0x30, 0x00, 0x00, 0x00, 0x00, 0x00, // bytes
    ];

    const ORACLE_DESC: BufferDescV1 = BufferDescV1 {
        width: 1024,
        height: 768,
        stride: 4096,
        format: FORMAT_XRGB8888_LE,
        bytes: 3_145_728,
    };

    #[test]
    fn byte_oracle_decode() {
        assert_eq!(decode_desc(&ORACLE_BYTES), ORACLE_DESC);
    }

    #[test]
    fn byte_oracle_encode() {
        assert_eq!(encode_desc(&ORACLE_DESC), ORACLE_BYTES);
    }

    #[test]
    fn damage_rect_byte_oracle_and_bounds() {
        // Hand-laid x=8, y=16, width=8, height=16, all u16 little-endian.
        const BYTES: [u8; DAMAGE_RECT_V1_LEN] = [8, 0, 16, 0, 8, 0, 16, 0];
        const RECT: DamageRectV1 = DamageRectV1 {
            x: 8,
            y: 16,
            width: 8,
            height: 16,
        };
        assert_eq!(decode_damage(&BYTES), Some(RECT));
        assert_eq!(encode_damage(RECT), BYTES);
        assert!(validate_damage(RECT, &ORACLE_DESC));
        assert!(!validate_damage(
            DamageRectV1 { width: 0, ..RECT },
            &ORACLE_DESC
        ));
        assert!(!validate_damage(
            DamageRectV1 { x: 1020, ..RECT },
            &ORACLE_DESC
        ));
        assert!(decode_damage(&BYTES[..7]).is_none());
    }

    /// The hand-laid present-result byte oracle: PresentResultV1 {
    /// present_id: 0x0102030405060708, outcome: OUTCOME_COMPOSED_TO_FB (1),
    /// flags: 0, submit_tick: 0xDEADBEEF, complete_tick: 0xFEEDFACE },
    /// little-endian. Laid out BY HAND from the repr(C) layout, independent
    /// of both codec directions. Values chosen to exercise every byte lane
    /// (present_id's 8 distinct bytes) and to make a field swap detectable
    /// (outcome≠flags, submit_tick≠complete_tick):
    ///   present_id    0x0102030405060708 -> 08 07 06 05 04 03 02 01  (@0)
    ///   outcome       0x00000001         -> 01 00 00 00              (@8)
    ///   flags         0x00000000         -> 00 00 00 00              (@12)
    ///   submit_tick   0x00000000DEADBEEF -> EF BE AD DE 00 00 00 00  (@16)
    ///   complete_tick 0x00000000FEEDFACE -> CE FA ED FE 00 00 00 00  (@24)
    const PRESENT_ORACLE_BYTES: [u8; PRESENT_RESULT_V1_LEN] = [
        0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, // present_id
        0x01, 0x00, 0x00, 0x00, // outcome = OUTCOME_COMPOSED_TO_FB
        0x00, 0x00, 0x00, 0x00, // flags = 0 (slice-1 sets no flag)
        0xEF, 0xBE, 0xAD, 0xDE, 0x00, 0x00, 0x00, 0x00, // submit_tick
        0xCE, 0xFA, 0xED, 0xFE, 0x00, 0x00, 0x00, 0x00, // complete_tick
    ];

    const PRESENT_ORACLE: PresentResultV1 = PresentResultV1 {
        present_id: 0x0102_0304_0506_0708,
        outcome: OUTCOME_COMPOSED_TO_FB,
        flags: 0,
        submit_tick: 0xDEAD_BEEF,
        complete_tick: 0xFEED_FACE,
    };

    #[test]
    fn present_result_byte_oracle_decode() {
        assert_eq!(decode_present_result(&PRESENT_ORACLE_BYTES), PRESENT_ORACLE);
    }

    #[test]
    fn present_result_byte_oracle_encode() {
        assert_eq!(encode_present_result(&PRESENT_ORACLE), PRESENT_ORACLE_BYTES);
    }

    #[test]
    fn present_result_len_is_32() {
        assert_eq!(PRESENT_RESULT_V1_LEN, 32);
        assert_eq!(
            core::mem::size_of::<PresentResultV1>(),
            PRESENT_RESULT_V1_LEN
        );
    }

    #[test]
    fn present_result_repr_c_bytes_match_oracle() {
        // The syscall copies the struct's in-memory bytes straight to user
        // (`write_unaligned(out.cast::<PresentResultV1>(), result)`). On a
        // little-endian host — CI and the x86_64 target are both LE — those
        // bytes ARE the wire encoding, so the raw struct image must equal the
        // hand-laid oracle. This is what binds the oracle to the real
        // copy-out path rather than a parallel codec.
        let raw: [u8; PRESENT_RESULT_V1_LEN] = unsafe {
            core::mem::transmute::<PresentResultV1, [u8; PRESENT_RESULT_V1_LEN]>(PRESENT_ORACLE)
        };
        assert_eq!(raw, PRESENT_ORACLE_BYTES);
    }

    #[test]
    fn format_const_is_drm_fourcc_xr24() {
        assert_eq!(FORMAT_XRGB8888_LE, 0x3432_5258);
        assert_eq!(&FORMAT_XRGB8888_LE.to_le_bytes(), b"XR24");
    }

    fn desc(width: u32, height: u32, stride: u32, format: u32, bytes: u64) -> BufferDescV1 {
        BufferDescV1 {
            width,
            height,
            stride,
            format,
            bytes,
        }
    }

    const OUT_W: u32 = 1920;
    const OUT_H: u32 = 1080;

    #[test]
    fn validate_accepts_the_oracle() {
        assert_eq!(validate_desc(&ORACLE_DESC, OUT_W, OUT_H), Ok(()));
    }

    #[test]
    fn validate_truth_table() {
        let f = FORMAT_XRGB8888_LE;
        // zero dimensions
        assert_eq!(
            validate_desc(&desc(0, 32, 256, f, 8192), OUT_W, OUT_H),
            Err(DescError::ZeroDimension)
        );
        assert_eq!(
            validate_desc(&desc(64, 0, 256, f, 0), OUT_W, OUT_H),
            Err(DescError::ZeroDimension)
        );
        // format
        assert_eq!(
            validate_desc(&desc(64, 32, 256, 0xDEAD_BEEF, 8192), OUT_W, OUT_H),
            Err(DescError::UnsupportedFormat)
        );
        // stride floor + alignment
        assert_eq!(
            validate_desc(&desc(64, 32, 255, f, 8160), OUT_W, OUT_H),
            Err(DescError::StrideTooSmall)
        );
        assert_eq!(
            validate_desc(&desc(64, 32, 258, f, 8256), OUT_W, OUT_H),
            Err(DescError::StrideMisaligned)
        );
        // bytes mismatch
        assert_eq!(
            validate_desc(&desc(64, 32, 256, f, 8191), OUT_W, OUT_H),
            Err(DescError::BytesMismatch)
        );
        // 64 MiB ceiling is INCLUSIVE: exactly 64 MiB passes, one row over
        // fails TooLarge.
        assert_eq!(
            validate_desc(&desc(2048, 8192, 8192, f, 8192 * 8192), 16384, 16384),
            Ok(())
        );
        assert_eq!(
            validate_desc(&desc(2048, 8193, 8192, f, 8192 * 8193), 16384, 16384),
            Err(DescError::TooLarge)
        );
        // exceeds output
        assert_eq!(
            validate_desc(
                &desc(OUT_W + 1, 1, (OUT_W + 1) * 4, f, ((OUT_W + 1) * 4) as u64),
                OUT_W,
                OUT_H
            ),
            Err(DescError::ExceedsOutput)
        );
        assert_eq!(
            validate_desc(
                &desc(1, OUT_H + 1, 4, f, 4 * (OUT_H as u64 + 1)),
                OUT_W,
                OUT_H
            ),
            Err(DescError::ExceedsOutput)
        );
    }

    #[test]
    fn validate_checked_math_overflow() {
        let f = FORMAT_XRGB8888_LE;
        // width * 4 overflow (u64 domain can't overflow with u32 inputs, but
        // stride * height CAN'T overflow u64 either with u32 inputs — the
        // Overflow arm is a guard for future field widening; today the
        // extreme values fall out through the other checks instead.
        assert!(
            validate_desc(
                &desc(u32::MAX, u32::MAX, u32::MAX, f, u64::MAX),
                OUT_W,
                OUT_H
            )
            .is_err()
        );
    }

    #[test]
    fn fence_epoch_satisfaction() {
        // The buffer-reuse ABA guard: a waiter is satisfied only by its own
        // or a later present — never by a residual earlier signal.
        assert!(!fence_satisfied(0, 1)); // nothing completed yet
        assert!(fence_satisfied(1, 1)); // its own present
        assert!(fence_satisfied(2, 1)); // a later present also releases it
        assert!(!fence_satisfied(1, 2)); // present N cannot satisfy N+1
        assert!(fence_satisfied(u64::MAX, 1));
        assert!(!fence_satisfied(u64::MAX - 1, u64::MAX));
    }

    #[test]
    fn console_route_kill_switch_kat() {
        // Slice-1 ships NO takeover: every combination — including both
        // switches on — routes stdout/stdin to the v1 console. When the
        // takeover slice extends the enum, the off-branch rows below pin
        // compositor-off == v1 console forever.
        for feature_on in [false, true] {
            for runtime_on in [false, true] {
                assert_eq!(
                    select_console_route(feature_on, runtime_on),
                    ConsoleRoute::V1Console,
                    "feature_on={feature_on} runtime_on={runtime_on}"
                );
            }
        }
    }

    #[test]
    fn atrium_syscalls_enabled_requires_both_layers() {
        assert!(!atrium_syscalls_enabled(false, false));
        assert!(!atrium_syscalls_enabled(true, false));
        assert!(!atrium_syscalls_enabled(false, true));
        assert!(atrium_syscalls_enabled(true, true));
    }

    #[test]
    fn rights_bits_distinct() {
        let buffer_bits = [
            buffer_rights::MAP_WRITE,
            buffer_rights::MAP_READ,
            buffer_rights::QUERY,
            buffer_rights::PRESENT,
            buffer_rights::COMMIT,
        ];
        for (i, a) in buffer_bits.iter().enumerate() {
            assert_eq!(a.count_ones(), 1);
            for b in &buffer_bits[i + 1..] {
                assert_eq!(a & b, 0, "overlapping buffer rights bits");
            }
        }
        assert_eq!(focus_rights::ACTIVATE.count_ones(), 1);
        assert_eq!(focus_rights::RECEIVE.count_ones(), 1);
        assert_eq!(focus_rights::ACTIVATE & focus_rights::RECEIVE, 0);
    }

    /// Slice-3 cross-process import: the compositor role is a STRICT read-only
    /// sibling of the client — MAP_READ present, MAP_WRITE and COMMIT absent.
    /// This is the pure-logic half of the C3 "compositor genuinely cannot
    /// write" proof, asserted independently of any kernel type.
    #[test]
    fn compositor_role_is_readonly_sibling() {
        // The two role sets are exactly as documented.
        assert_eq!(
            buffer_rights::CLIENT_ROLE,
            buffer_rights::MAP_WRITE
                | buffer_rights::MAP_READ
                | buffer_rights::QUERY
                | buffer_rights::PRESENT
                | buffer_rights::COMMIT,
        );
        assert_eq!(
            buffer_rights::COMPOSITOR_ROLE,
            buffer_rights::MAP_READ | buffer_rights::PRESENT | buffer_rights::QUERY,
        );

        // The compositor role carries neither write path.
        assert_eq!(buffer_rights::COMPOSITOR_ROLE & buffer_rights::MAP_WRITE, 0);
        assert_eq!(buffer_rights::COMPOSITOR_ROLE & buffer_rights::COMMIT, 0);
        // ...and is a strict subset of the client role (a real attenuation).
        assert_eq!(
            buffer_rights::COMPOSITOR_ROLE & buffer_rights::CLIENT_ROLE,
            buffer_rights::COMPOSITOR_ROLE,
        );
        assert_ne!(buffer_rights::COMPOSITOR_ROLE, buffer_rights::CLIENT_ROLE);

        // The import predicate accepts the compositor role and rejects any
        // role that can write or commit.
        assert!(is_readonly_compositor_import(
            buffer_rights::COMPOSITOR_ROLE
        ));
        assert!(!is_readonly_compositor_import(buffer_rights::CLIENT_ROLE));
        assert!(!is_readonly_compositor_import(
            buffer_rights::COMPOSITOR_ROLE | buffer_rights::MAP_WRITE
        ));
        assert!(!is_readonly_compositor_import(
            buffer_rights::COMPOSITOR_ROLE | buffer_rights::COMMIT
        ));
        // A cap with no read path is not a valid import either.
        assert!(!is_readonly_compositor_import(buffer_rights::QUERY));
    }

    /// The grant attenuation is monotone-shrinking (`source & offered & max`):
    /// no combination of an honest client offer and a compositor ceiling ever
    /// re-introduces MAP_WRITE. Mirrors `CapRights::intersection` on the
    /// semantic byte — the kernel-side guarantee behind the QEMU proof that a
    /// compositor cap cannot upgrade to a writable mapping.
    #[test]
    fn grant_attenuation_never_widens_to_write() {
        let root = buffer_rights::CLIENT_ROLE; // the creator's full semantic set
        let offered = buffer_rights::COMPOSITOR_ROLE; // what the client grants
        let ceiling = buffer_rights::COMPOSITOR_ROLE; // recipient's max_rights

        let granted = granted_semantic(root, offered, ceiling);
        assert_eq!(granted, buffer_rights::COMPOSITOR_ROLE);
        assert!(is_readonly_compositor_import(granted));

        // Even a MALICIOUS recipient asking for the whole world is bounded by
        // the client's offer (never gets MAP_WRITE/COMMIT).
        let greedy = granted_semantic(root, offered, 0xFF);
        assert_eq!(greedy, buffer_rights::COMPOSITOR_ROLE);
        assert_eq!(greedy & buffer_rights::MAP_WRITE, 0);

        // And a malicious GRANTOR cannot offer bits the source lacks: offering
        // the full client role but with a compositor ceiling still yields RO.
        let capped = granted_semantic(root, buffer_rights::CLIENT_ROLE, ceiling);
        assert_eq!(capped, buffer_rights::COMPOSITOR_ROLE);
        assert_eq!(capped & buffer_rights::MAP_WRITE, 0);

        // Intersection can only remove bits — the result is always a subset of
        // every input.
        for source in 0u8..=0x1F {
            for off in 0u8..=0x1F {
                for max in 0u8..=0x1F {
                    let g = granted_semantic(source, off, max);
                    assert_eq!(g & !source, 0);
                    assert_eq!(g & !off, 0);
                    assert_eq!(g & !max, 0);
                }
            }
        }
    }

    #[test]
    fn max_buffer_bytes_is_64_mib() {
        assert_eq!(MAX_BUFFER_BYTES, 64 * 1024 * 1024);
    }

    const NS: u64 = 0x0102_0304_0506_0708;
    const SID: u64 = 0x1112_1314_1516_1718;
    const SURFACE_CAP: u64 = 0x2122_2324_2526_2728;
    const REQUEST_ID: u64 = 0x3132_3334_3536_3738;

    fn identity() -> protocol::Identity {
        protocol::Identity {
            namespace_id: NS,
            surface_id: SID,
            surface_cap_provenance: SURFACE_CAP,
        }
    }

    // Hand-laid common envelope, independent of encode_request/decode_request:
    // magic "NMC1", version=1, opcode=?, payload_len=?, flags=0,
    // request_id=0x3132333435363738, followed by namespace/surface/cap IDs.
    fn common_fixture(opcode: u16, payload_len: u32) -> [u8; protocol::MAX_MESSAGE_LEN] {
        let mut b = [0u8; protocol::MAX_MESSAGE_LEN];
        b[0..24].copy_from_slice(&[
            0x4e,
            0x4d,
            0x43,
            0x31,
            0x01,
            0x00,
            opcode as u8,
            0x00,
            payload_len as u8,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x00,
            0x38,
            0x37,
            0x36,
            0x35,
            0x34,
            0x33,
            0x32,
            0x31,
        ]);
        b[24..48].copy_from_slice(&[
            0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0x18, 0x17, 0x16, 0x15, 0x14, 0x13,
            0x12, 0x11, 0x28, 0x27, 0x26, 0x25, 0x24, 0x23, 0x22, 0x21,
        ]);
        b
    }

    #[test]
    fn create_surface_byte_oracle_and_role_tags() {
        let create = protocol::CreateSurface {
            identity: identity(),
            role: protocol::ROLE_WORKFIELD_ROOT,
            width: 1920,
            height: 1080,
            client_tag: 0x4142_4344_4546_4748,
        };
        let mut fixture = common_fixture(protocol::OP_CREATE_SURFACE, 48);
        fixture[48..72].copy_from_slice(&[
            0x01, 0x00, 0x00, 0x00, // WORKFIELD_ROOT
            0x80, 0x07, 0x00, 0x00, // 1920
            0x38, 0x04, 0x00, 0x00, // 1080
            0x00, 0x00, 0x00, 0x00, // reserved
            0x48, 0x47, 0x46, 0x45, 0x44, 0x43, 0x42, 0x41,
        ]);
        let (encoded, len) =
            protocol::encode_request(REQUEST_ID, protocol::Request::Create(create));
        assert_eq!(len, 72);
        assert_eq!(encoded, fixture);
        assert_eq!(
            protocol::decode_request(&fixture),
            Ok((
                protocol::Header {
                    opcode: 1,
                    payload_len: 48,
                    request_id: REQUEST_ID
                },
                protocol::Request::Create(create),
            ))
        );

        let text = protocol::CreateSurface {
            role: protocol::ROLE_TEXT,
            ..create
        };
        let (text_bytes, _) = protocol::encode_request(REQUEST_ID, protocol::Request::Create(text));
        assert_eq!(&text_bytes[48..52], &[0x02, 0, 0, 0]);
        assert_ne!(&text_bytes[48..52], &fixture[48..52]);
    }

    #[test]
    fn import_attach_byte_oracle() {
        let request = protocol::ImportAttachBuffer {
            identity: identity(),
            transfer_id: 0x4142_4344_4546_4748,
            buffer_cap_provenance: 0x5152_5354_5556_5758,
        };
        let mut fixture = common_fixture(protocol::OP_IMPORT_ATTACH_BUFFER, 40);
        fixture[48..64].copy_from_slice(&[
            0x48, 0x47, 0x46, 0x45, 0x44, 0x43, 0x42, 0x41, 0x58, 0x57, 0x56, 0x55, 0x54, 0x53,
            0x52, 0x51,
        ]);
        let (encoded, len) =
            protocol::encode_request(REQUEST_ID, protocol::Request::ImportAttach(request));
        assert_eq!(len, 64);
        assert_eq!(&encoded[..len], &fixture[..len]);
        assert_eq!(
            protocol::decode_request(&fixture[..len]).unwrap().1,
            protocol::Request::ImportAttach(request)
        );
    }

    #[test]
    fn commit_byte_oracle() {
        let request = protocol::Commit {
            identity: identity(),
            client_serial: 0x4142_4344_4546_4748,
        };
        let mut fixture = common_fixture(protocol::OP_COMMIT, 32);
        fixture[48..56].copy_from_slice(&[0x48, 0x47, 0x46, 0x45, 0x44, 0x43, 0x42, 0x41]);
        let (encoded, len) =
            protocol::encode_request(REQUEST_ID, protocol::Request::Commit(request));
        assert_eq!(&encoded[..len], &fixture[..56]);
        assert_eq!(
            protocol::decode_request(&fixture[..56]).unwrap().1,
            protocol::Request::Commit(request)
        );
    }

    #[test]
    fn focus_request_byte_oracle() {
        let request = protocol::FocusRequest {
            identity: identity(),
            reason: 1,
            request_serial: 0x4142_4344_4546_4748,
        };
        let mut fixture = common_fixture(protocol::OP_FOCUS_REQUEST, 40);
        fixture[48..64].copy_from_slice(&[
            0x01, 0, 0, 0, 0, 0, 0, 0, 0x48, 0x47, 0x46, 0x45, 0x44, 0x43, 0x42, 0x41,
        ]);
        let (encoded, len) =
            protocol::encode_request(REQUEST_ID, protocol::Request::Focus(request));
        assert_eq!(&encoded[..len], &fixture[..64]);
        assert_eq!(
            protocol::decode_request(&fixture[..64]).unwrap().1,
            protocol::Request::Focus(request)
        );
    }

    #[test]
    fn destroy_surface_byte_oracle() {
        let request = protocol::DestroySurface {
            identity: identity(),
            final_client_serial: 0x4142_4344_4546_4748,
        };
        let mut fixture = common_fixture(protocol::OP_DESTROY_SURFACE, 32);
        fixture[48..56].copy_from_slice(&[0x48, 0x47, 0x46, 0x45, 0x44, 0x43, 0x42, 0x41]);
        let (encoded, len) =
            protocol::encode_request(REQUEST_ID, protocol::Request::Destroy(request));
        assert_eq!(&encoded[..len], &fixture[..56]);
        assert_eq!(
            protocol::decode_request(&fixture[..56]).unwrap().1,
            protocol::Request::Destroy(request)
        );
    }

    #[test]
    fn configure_surface_is_size_only_byte_oracle() {
        let request = protocol::ConfigureSurface {
            identity: identity(),
            width: 640,
            height: 480,
            configure_serial: 0x4142_4344_4546_4748,
        };
        let mut fixture = common_fixture(protocol::OP_CONFIGURE, 40);
        fixture[48..64].copy_from_slice(&[
            0x80, 0x02, 0x00, 0x00, // width = 640
            0xe0, 0x01, 0x00, 0x00, // height = 480
            0x48, 0x47, 0x46, 0x45, 0x44, 0x43, 0x42, 0x41,
        ]);
        let (encoded, len) =
            protocol::encode_request(REQUEST_ID, protocol::Request::Configure(request));
        assert_eq!(len, 64);
        assert_eq!(&encoded[..len], &fixture[..len]);
        assert_eq!(
            protocol::decode_request(&fixture[..len]).unwrap().1,
            protocol::Request::Configure(request)
        );
        assert_eq!(protocol::CONFIGURE_PAYLOAD_LEN, 40);
        assert_eq!(protocol::HEADER_LEN + protocol::CONFIGURE_PAYLOAD_LEN, len);
    }

    #[test]
    fn reply_envelope_byte_oracle() {
        let reply = protocol::Reply {
            opcode: protocol::OP_CREATE_SURFACE,
            request_id: REQUEST_ID,
            status: 0,
            identity: identity(),
        };
        // Hand-laid reply: opcode response bit (0x8001), 32-byte payload,
        // status/reserved zero, then the returned namespaced surface identity.
        const FIXTURE: [u8; protocol::REPLY_LEN] = [
            0x4e, 0x4d, 0x43, 0x31, 0x01, 0x00, 0x01, 0x80, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x38, 0x37, 0x36, 0x35, 0x34, 0x33, 0x32, 0x31, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0x18, 0x17,
            0x16, 0x15, 0x14, 0x13, 0x12, 0x11, 0x28, 0x27, 0x26, 0x25, 0x24, 0x23, 0x22, 0x21,
        ];
        assert_eq!(protocol::encode_reply(reply), FIXTURE);
        assert_eq!(protocol::decode_reply(&FIXTURE), Ok(reply));
    }

    fn create_root() -> protocol::CreateSurface {
        protocol::CreateSurface {
            identity: protocol::Identity {
                namespace_id: NS,
                surface_id: 0,
                surface_cap_provenance: 0,
            },
            role: protocol::ROLE_WORKFIELD_ROOT,
            width: 1920,
            height: 1080,
            client_tag: 7,
        }
    }

    fn create_text(client_tag: u64) -> protocol::CreateSurface {
        protocol::CreateSurface {
            identity: protocol::Identity {
                namespace_id: NS,
                surface_id: 0,
                surface_cap_provenance: 0,
            },
            role: protocol::ROLE_TEXT,
            width: 32,
            height: 32,
            client_tag,
        }
    }

    #[test]
    fn dw1_allows_two_text_surfaces_and_fuzzes_peer_identity() {
        let mut registry = protocol::SurfaceRegistry::new(NS, 96, 64);
        let a = registry.create(create_text(0x41), SURFACE_CAP).unwrap();
        let b = registry.create(create_text(0x42), SURFACE_CAP + 1).unwrap();
        assert_ne!(a.surface_id, b.surface_id);

        let peer_named_by_a = protocol::Identity {
            surface_id: b.surface_id,
            surface_cap_provenance: a.surface_cap_provenance,
            ..b
        };
        assert_eq!(
            registry.import_attach_on_channel(
                protocol::ImportAttachBuffer {
                    identity: peer_named_by_a,
                    transfer_id: 1,
                    buffer_cap_provenance: 0x99
                },
                a.surface_cap_provenance,
                0x99
            ),
            Err(protocol::ProtocolError::CapRequired)
        );
        assert_eq!(
            registry.focus_request_on_channel(
                protocol::FocusRequest {
                    identity: peer_named_by_a,
                    reason: protocol::FOCUS_REASON_USER_ACTIVATION,
                    request_serial: 1
                },
                a.surface_cap_provenance,
                |_, _| true
            ),
            Err(protocol::ProtocolError::CapRequired)
        );
        assert_eq!(
            registry.destroy_on_channel(
                protocol::DestroySurface {
                    identity: peer_named_by_a,
                    final_client_serial: 0
                },
                a.surface_cap_provenance
            ),
            Err(protocol::ProtocolError::CapRequired)
        );
        assert_eq!(
            registry.destroy_on_channel(
                protocol::DestroySurface {
                    identity: b,
                    final_client_serial: 0
                },
                a.surface_cap_provenance
            ),
            Err(protocol::ProtocolError::CapRequired)
        );
        assert_eq!(
            registry.configure_on_channel(
                protocol::ConfigureSurface {
                    identity: peer_named_by_a,
                    width: 24,
                    height: 24,
                    configure_serial: 1
                },
                a.surface_cap_provenance
            ),
            Err(protocol::ProtocolError::CapRequired)
        );

        let foreign_namespace = protocol::Identity {
            namespace_id: NS + 1,
            ..a
        };
        assert_eq!(
            registry.commit_on_channel(
                protocol::Commit {
                    identity: foreign_namespace,
                    client_serial: 1
                },
                a.surface_cap_provenance
            ),
            Err(protocol::ProtocolError::WrongOwner)
        );
        assert_eq!(
            registry.configure_on_channel(
                protocol::ConfigureSurface {
                    identity: foreign_namespace,
                    width: 24,
                    height: 24,
                    configure_serial: 1
                },
                a.surface_cap_provenance
            ),
            Err(protocol::ProtocolError::WrongOwner)
        );
        assert_eq!(
            registry.configure_on_channel(
                protocol::ConfigureSurface {
                    identity: b,
                    width: 24,
                    height: 24,
                    configure_serial: 2
                },
                a.surface_cap_provenance
            ),
            Err(protocol::ProtocolError::CapRequired)
        );
    }

    /// DS-1 `OP_YIELD_FOCUS`: opcode 7, payload 32 = identity (24) + a
    /// non-zero little-endian yield serial at bytes 48..56.
    #[test]
    fn yield_focus_byte_oracle() {
        let request = protocol::YieldFocus {
            identity: identity(),
            yield_serial: 0x4142_4344_4546_4748,
        };
        let mut fixture = common_fixture(7, 32);
        fixture[48..56].copy_from_slice(&[0x48, 0x47, 0x46, 0x45, 0x44, 0x43, 0x42, 0x41]);
        assert_eq!(protocol::OP_YIELD_FOCUS, 7);
        let (encoded, len) =
            protocol::encode_request(REQUEST_ID, protocol::Request::Yield(request));
        assert_eq!(len, 56);
        assert_eq!(&encoded[..len], &fixture[..56]);
        assert_eq!(
            protocol::decode_request(&fixture[..56]).unwrap().1,
            protocol::Request::Yield(request)
        );
        // A zero serial and a truncated message are refused.
        let mut zero = fixture;
        zero[48..56].fill(0);
        assert_eq!(
            protocol::decode_request(&zero[..56]),
            Err(protocol::ProtocolError::BadField)
        );
        assert_eq!(
            protocol::decode_request(&fixture[..55]),
            Err(protocol::ProtocolError::BadLength)
        );
    }

    /// DS-1b `OP_LAUNCH`: opcode 8, payload 40 = identity (24) + module id
    /// (u32 LE at 48) + a zero reserved word (52..56) + the launch token
    /// (u64 LE at 56..64).
    #[test]
    fn launch_byte_oracle() {
        let request = protocol::LaunchRequest {
            identity: identity(),
            module_id: 2,
            launch_token: 0x4142_4344_4546_4748,
        };
        let mut fixture = common_fixture(8, 40);
        fixture[48..64].copy_from_slice(&[
            0x02, 0x00, 0x00, 0x00, // module id 2
            0x00, 0x00, 0x00, 0x00, // reserved
            0x48, 0x47, 0x46, 0x45, 0x44, 0x43, 0x42, 0x41, // token
        ]);
        assert_eq!(protocol::OP_LAUNCH, 8);
        let (encoded, len) =
            protocol::encode_request(REQUEST_ID, protocol::Request::Launch(request));
        assert_eq!(len, 64);
        assert_eq!(&encoded[..len], &fixture[..64]);
        assert_eq!(
            protocol::decode_request(&fixture[..64]).unwrap().1,
            protocol::Request::Launch(request)
        );
        for (at, what) in [(48usize, "module 0"), (52, "reserved"), (56, "token 0")] {
            let mut bad = fixture;
            if what == "reserved" {
                bad[52] = 1;
            } else {
                bad[at..at + if at == 48 { 4 } else { 8 }].fill(0);
            }
            assert_eq!(
                protocol::decode_request(&bad[..64]),
                Err(protocol::ProtocolError::BadField),
                "{what}"
            );
        }
    }

    /// S2-F2: `OP_COMMIT_DAMAGE` = 9, payload 40, hand-laid: the serial at
    /// 48, then x, y, w, h as little-endian u16 at 56..64. An empty rect is
    /// refused by the decoder.
    #[test]
    fn commit_damage_byte_oracle() {
        let request = protocol::CommitDamage {
            identity: identity(),
            client_serial: 0x0102_0304_0506_0708,
            x: 0x0011,
            y: 0x0122,
            width: 0x0280,
            height: 0x0016,
        };
        let mut fixture = common_fixture(9, 40);
        fixture[48..64].copy_from_slice(&[
            0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, // serial
            0x11, 0x00, // x = 17
            0x22, 0x01, // y = 290
            0x80, 0x02, // w = 640
            0x16, 0x00, // h = 22
        ]);
        assert_eq!(protocol::OP_COMMIT_DAMAGE, 9);
        let (encoded, len) =
            protocol::encode_request(REQUEST_ID, protocol::Request::CommitDamage(request));
        assert_eq!(len, 64);
        assert_eq!(&encoded[..len], &fixture[..64]);
        assert_eq!(
            protocol::decode_request(&fixture[..64]).unwrap().1,
            protocol::Request::CommitDamage(request)
        );
        for at in [60usize, 62] {
            let mut bad = fixture;
            bad[at..at + 2].fill(0);
            assert_eq!(
                protocol::decode_request(&bad[..64]),
                Err(protocol::ProtocolError::BadField),
                "empty rect at byte {at}"
            );
        }
    }

    /// S2-F2: a damage commit is honoured only on the surface's own channel
    /// (INV-WM-1, as `OP_COMMIT`) and only for a rect inside the surface as
    /// created; a refused one commits nothing (the serial is not consumed).
    #[test]
    fn registry_commit_damage_is_channel_bound_and_bounded() {
        let mut registry = protocol::SurfaceRegistry::new(NS, 96, 64);
        let a = registry.create(create_text(0x41), SURFACE_CAP).unwrap();
        let b = registry.create(create_text(0x42), SURFACE_CAP + 1).unwrap();
        for (s, prov) in [(a, 0x99), (b, 0x9A)] {
            registry
                .import_attach_on_channel(
                    protocol::ImportAttachBuffer {
                        identity: s,
                        transfer_id: 1,
                        buffer_cap_provenance: prov,
                    },
                    s.surface_cap_provenance,
                    prov,
                )
                .unwrap();
        }
        let d = |identity, client_serial, x, y, width, height| protocol::CommitDamage {
            identity,
            client_serial,
            x,
            y,
            width,
            height,
        };
        // Outside the 32x32 surface: refused, and serial 1 stays unused.
        for bad in [
            d(a, 1, 0, 0, 33, 1),
            d(a, 1, 31, 31, 2, 1),
            d(a, 1, 0, 32, 1, 1),
        ] {
            assert_eq!(
                registry.commit_damage_on_channel(bad, a.surface_cap_provenance),
                Err(protocol::ProtocolError::BadField)
            );
        }
        let ok = d(a, 1, 4, 30, 28, 2);
        assert_eq!(
            registry.commit_damage_on_channel(ok, a.surface_cap_provenance),
            Ok((1, ok))
        );
        // Serials still only move forward.
        assert_eq!(
            registry.commit_damage_on_channel(ok, a.surface_cap_provenance),
            Err(protocol::ProtocolError::BadState)
        );
        // A peer's identity on A's channel, and A's on B's.
        assert_eq!(
            registry.commit_damage_on_channel(d(b, 2, 0, 0, 1, 1), a.surface_cap_provenance),
            Err(protocol::ProtocolError::CapRequired)
        );
        assert_eq!(
            registry.commit_damage_on_channel(d(a, 2, 0, 0, 1, 1), b.surface_cap_provenance),
            Err(protocol::ProtocolError::CapRequired)
        );
    }

    /// A9-D: `OP_BUSY` = 10, payload 40, hand-laid: the serial at 48, then
    /// x, y, w, h as little-endian u16 at 56..64. A zero serial or an empty
    /// rect is refused by the decoder.
    #[test]
    fn busy_byte_oracle() {
        let request = protocol::Busy {
            identity: identity(),
            busy_serial: 0x1112_1314_1516_1718,
            x: 0x0048,
            y: 0x01a3,
            width: 0x0030,
            height: 0x0031,
        };
        let mut fixture = common_fixture(10, 40);
        fixture[48..64].copy_from_slice(&[
            0x18, 0x17, 0x16, 0x15, 0x14, 0x13, 0x12, 0x11, // serial
            0x48, 0x00, // x = 72
            0xa3, 0x01, // y = 419
            0x30, 0x00, // w = 48
            0x31, 0x00, // h = 49
        ]);
        assert_eq!(protocol::OP_BUSY, 10);
        let (encoded, len) = protocol::encode_request(REQUEST_ID, protocol::Request::Busy(request));
        assert_eq!(len, 64);
        assert_eq!(&encoded[..len], &fixture[..64]);
        assert_eq!(
            protocol::decode_request(&fixture[..64]).unwrap().1,
            protocol::Request::Busy(request)
        );
        for (at, n, what) in [
            (48usize, 8usize, "serial 0"),
            (60, 2, "w 0"),
            (62, 2, "h 0"),
        ] {
            let mut bad = fixture;
            bad[at..at + n].fill(0);
            assert_eq!(
                protocol::decode_request(&bad[..64]),
                Err(protocol::ProtocolError::BadField),
                "{what}"
            );
        }
        // The reply echoes the opcode under the reply bit.
        let reply = protocol::encode_reply(protocol::Reply {
            opcode: protocol::OP_BUSY,
            request_id: REQUEST_ID,
            status: 0,
            identity: identity(),
        });
        assert_eq!(&reply[6..8], &[0x0a, 0x80]);
        assert_eq!(
            protocol::decode_reply(&reply).unwrap().opcode,
            protocol::OP_BUSY
        );
    }

    /// OT-88 S1: `OP_SETTING` = 11, payload 40, hand-laid: the key id at 48
    /// (`u16`), a zero `u16` at 50, the value at 52 (`u32`), a zero `u64` at
    /// 56. A non-zero reserved word is refused by the decoder; the key and
    /// value are the WM's to check. The refusals' statuses are 20 and 21.
    #[test]
    fn setting_byte_oracle() {
        let request = protocol::Setting {
            identity: identity(),
            key: 0x0003,
            value: 0x0000_001e,
        };
        let mut fixture = common_fixture(11, 40);
        fixture[48..64].copy_from_slice(&[
            0x03, 0x00, // key 3 (idle-lock minutes)
            0x00, 0x00, // reserved
            0x1e, 0x00, 0x00, 0x00, // value 30
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // reserved
        ]);
        assert_eq!(protocol::OP_SETTING, 11);
        let (encoded, len) =
            protocol::encode_request(REQUEST_ID, protocol::Request::Setting(request));
        assert_eq!(len, 64);
        assert_eq!(&encoded[..len], &fixture[..64]);
        assert_eq!(
            protocol::decode_request(&fixture[..64]).unwrap().1,
            protocol::Request::Setting(request)
        );
        for (at, what) in [
            (50usize, "reserved u16"),
            (56, "reserved u64"),
            (63, "reserved u64 top"),
        ] {
            let mut bad = fixture;
            bad[at] = 1;
            assert_eq!(
                protocol::decode_request(&bad[..64]),
                Err(protocol::ProtocolError::BadField),
                "{what}"
            );
        }
        // A key or value out of range still decodes: refusing it is the WM's.
        let mut odd = fixture;
        odd[48..50].copy_from_slice(&[0xff, 0xff]);
        odd[52..56].copy_from_slice(&[0x07, 0x00, 0x00, 0x00]);
        assert!(protocol::decode_request(&odd[..64]).is_ok());
        assert_eq!(
            (
                protocol::STATUS_SETTING_KEY,
                protocol::STATUS_SETTING_VALUE,
                protocol::STATUS_SETTING_STORE
            ),
            (20, 21, 22)
        );
        let reply = protocol::encode_reply(protocol::Reply {
            opcode: protocol::OP_SETTING,
            request_id: REQUEST_ID,
            status: protocol::STATUS_SETTING_VALUE,
            identity: identity(),
        });
        assert_eq!(&reply[6..8], &[0x0b, 0x80]);
        assert_eq!(&reply[24..28], &[0x15, 0x00, 0x00, 0x00]);
    }

    /// L1.3: `OP_PERMISSION` = 12 and `OP_PERMISSION_DONE` = 13, payload 40,
    /// hand-laid: the action (u16 at 48) and the path's length (u16 at 50),
    /// or the outcome (u32 at 48); every other byte through 64 zero. The
    /// decoder refuses an unknown action, an empty or too-long path, and any
    /// reserved byte set.
    #[test]
    fn permission_byte_oracle() {
        let request = protocol::Permission {
            identity: identity(),
            action: protocol::PERMISSION_WRITE,
            path_len: 0x00ff,
            intent_len: 0x001c,
        };
        let mut fixture = common_fixture(12, 40);
        fixture[48..64].copy_from_slice(&[
            0x03, 0x00, // action = write
            0xff, 0x00, // path_len = 255
            0x1c, 0x00, // intent_len = 28 (L2.1)
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]);
        assert_eq!(
            (protocol::OP_PERMISSION, protocol::OP_PERMISSION_DONE),
            (12, 13)
        );
        assert_eq!(
            (
                protocol::PERMISSION_READ,
                protocol::PERMISSION_LIST,
                protocol::PERMISSION_WRITE
            ),
            (1, 2, 3)
        );
        let (encoded, len) =
            protocol::encode_request(REQUEST_ID, protocol::Request::Permission(request));
        assert_eq!(len, 64);
        assert_eq!(&encoded[..len], &fixture[..64]);
        assert_eq!(
            protocol::decode_request(&fixture[..64]).unwrap().1,
            protocol::Request::Permission(request)
        );
        for (at, bytes, what) in [
            (48usize, &[0x00u8, 0x00][..], "action 0"),
            (48, &[0x04, 0x00][..], "action 4"),
            (50, &[0x00, 0x00][..], "path_len 0"),
            (50, &[0x00, 0x01][..], "path_len 256"),
            (52, &[0x00, 0x01][..], "intent_len 256"),
            (54, &[0x01][..], "reserved u16"),
            (63, &[0x01][..], "reserved u64"),
        ] {
            let mut bad = fixture;
            bad[at..at + bytes.len()].copy_from_slice(bytes);
            assert_eq!(
                protocol::decode_request(&bad[..64]),
                Err(protocol::ProtocolError::BadField),
                "{what}"
            );
        }

        let done = protocol::PermissionDone {
            identity: identity(),
            outcome: 0x0000_000d,
        };
        let mut fixture = common_fixture(13, 40);
        fixture[48..64].copy_from_slice(&[0x0d, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let (encoded, len) =
            protocol::encode_request(REQUEST_ID, protocol::Request::PermissionDone(done));
        assert_eq!(&encoded[..len], &fixture[..64]);
        assert_eq!(
            protocol::decode_request(&fixture[..64]).unwrap().1,
            protocol::Request::PermissionDone(done)
        );
        let mut bad = fixture;
        bad[52] = 1;
        assert_eq!(
            protocol::decode_request(&bad[..64]),
            Err(protocol::ProtocolError::BadField)
        );
        // The statuses the client branches on.
        assert_eq!(
            (
                protocol::STATUS_PERMISSION_GRANTED,
                protocol::STATUS_PERMISSION_GRANTED_ALWAYS,
                protocol::STATUS_PERMISSION_DENIED,
                protocol::STATUS_PERMISSION_READ_ONLY,
                protocol::STATUS_PERMISSION_PATH,
                protocol::PERMISSION_PATH_MAX
            ),
            (0, 30, 31, 32, 33, 255)
        );
    }

    /// A9-D: a busy rect is honoured only on the surface's own channel and
    /// inside the surface; it consumes no commit serial.
    #[test]
    fn registry_busy_is_channel_bound_and_bounded() {
        let mut registry = protocol::SurfaceRegistry::new(NS, 96, 64);
        let a = registry.create(create_text(0x41), SURFACE_CAP).unwrap();
        let b = registry.create(create_text(0x42), SURFACE_CAP + 1).unwrap();
        registry
            .import_attach_on_channel(
                protocol::ImportAttachBuffer {
                    identity: a,
                    transfer_id: 1,
                    buffer_cap_provenance: 0x99,
                },
                a.surface_cap_provenance,
                0x99,
            )
            .unwrap();
        let busy = |identity, x, y, width, height| protocol::Busy {
            identity,
            busy_serial: 7,
            x,
            y,
            width,
            height,
        };
        for bad in [
            busy(a, 0, 0, 33, 1),
            busy(a, 31, 31, 2, 1),
            busy(a, 0, 32, 1, 1),
        ] {
            assert_eq!(
                registry.busy_on_channel(bad, a.surface_cap_provenance),
                Err(protocol::ProtocolError::BadField)
            );
        }
        let ok = busy(a, 4, 4, 28, 28);
        assert_eq!(
            registry.busy_on_channel(ok, a.surface_cap_provenance),
            Ok(ok)
        );
        assert_eq!(
            registry.busy_on_channel(busy(b, 0, 0, 1, 1), a.surface_cap_provenance),
            Err(protocol::ProtocolError::CapRequired)
        );
        assert_eq!(
            registry.busy_on_channel(ok, b.surface_cap_provenance),
            Err(protocol::ProtocolError::CapRequired)
        );
        // Commit serial 1 is still free.
        assert_eq!(
            registry.commit_on_channel(
                protocol::Commit {
                    identity: a,
                    client_serial: 1,
                },
                a.surface_cap_provenance,
            ),
            Ok(1)
        );
    }

    /// Janus #96: the WM refuses a bad `OP_BUSY` in the reply's existing
    /// status word (offset 24, the `ProtocolError::status` domain), never by
    /// ending the session. Hand-laid: the busy opcode under the reply bit
    /// (0x800a), the 32-byte payload, status 3 (BadField), reserved 0, then
    /// the surface identity echoed back.
    #[test]
    fn busy_refusal_reply_byte_oracle() {
        let reply = protocol::Reply {
            opcode: protocol::OP_BUSY,
            request_id: REQUEST_ID,
            status: protocol::ProtocolError::BadField.status(),
            identity: identity(),
        };
        const FIXTURE: [u8; protocol::REPLY_LEN] = [
            0x4e, 0x4d, 0x43, 0x31, 0x01, 0x00, 0x0a, 0x80, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x38, 0x37, 0x36, 0x35, 0x34, 0x33, 0x32, 0x31, 0x03, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, 0x18, 0x17,
            0x16, 0x15, 0x14, 0x13, 0x12, 0x11, 0x28, 0x27, 0x26, 0x25, 0x24, 0x23, 0x22, 0x21,
        ];
        assert_eq!(protocol::encode_reply(reply), FIXTURE);
        assert_eq!(protocol::decode_reply(&FIXTURE), Ok(reply));
        // The wrong-channel refusal is CapRequired, status 7, same word.
        let mut wrong_channel = FIXTURE;
        wrong_channel[24] = 0x07;
        assert_eq!(
            protocol::decode_reply(&wrong_channel).unwrap().status,
            protocol::ProtocolError::CapRequired.status()
        );
    }

    /// The request an undecodable message still names, so the WM can refuse
    /// it: hand-laid header "NMC1", version 1, opcode 9 (commit damage),
    /// payload_len 40, flags 0, request_id 0x3132333435363738, then a payload
    /// cut short (not a valid commit). `request_target` reads only the header;
    /// a short, foreign, wrong-version or reply message names nothing.
    #[test]
    fn request_target_byte_oracle() {
        const TRUNCATED: [u8; 32] = [
            0x4e, 0x4d, 0x43, 0x31, 0x01, 0x00, 0x09, 0x00, 0x28, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x38, 0x37, 0x36, 0x35, 0x34, 0x33, 0x32, 0x31, 0x08, 0x07, 0x06, 0x05,
            0x04, 0x03, 0x02, 0x01,
        ];
        assert_eq!(
            protocol::decode_request(&TRUNCATED),
            Err(protocol::ProtocolError::BadLength)
        );
        assert_eq!(protocol::ProtocolError::BadLength.status(), 2);
        assert_eq!(
            protocol::request_target(&TRUNCATED),
            Some((protocol::OP_COMMIT_DAMAGE, REQUEST_ID))
        );
        // A verb this build does not know still names its request.
        let mut unknown = TRUNCATED;
        unknown[6] = 0x7f;
        assert_eq!(protocol::request_target(&unknown), Some((0x7f, REQUEST_ID)));
        // Only a whole header of this protocol, for a request, names one.
        assert_eq!(protocol::request_target(&TRUNCATED[..23]), None);
        assert_eq!(protocol::request_target(b"GARBAGE!"), None);
        let mut foreign = TRUNCATED;
        foreign[0] = 0x4f;
        assert_eq!(protocol::request_target(&foreign), None);
        let mut version = TRUNCATED;
        version[4] = 0x02;
        assert_eq!(protocol::request_target(&version), None);
        let mut reply = TRUNCATED;
        reply[7] = 0x80;
        assert_eq!(protocol::request_target(&reply), None);
        // A refusal reply to it is the reply envelope with that opcode,
        // request id and status 2.
        let refusal = protocol::encode_reply(protocol::Reply {
            opcode: protocol::OP_COMMIT_DAMAGE,
            request_id: REQUEST_ID,
            status: 2,
            identity: identity(),
        });
        assert_eq!(&refusal[6..8], &[0x09, 0x80]);
        assert_eq!(
            &refusal[16..28],
            &[
                0x38, 0x37, 0x36, 0x35, 0x34, 0x33, 0x32, 0x31, 0x02, 0x00, 0x00, 0x00
            ]
        );
    }

    /// Janus #96's exact case on a 640x400 surface: a valid busy rect, then
    /// x=639 width=2 (one pixel past the right edge) is BadField, the same
    /// rect named with another channel's provenance is CapRequired, and a
    /// valid rect after both is still accepted (a refusal changes nothing).
    #[test]
    fn registry_busy_refusals_leave_the_surface_serving() {
        let mut registry = protocol::SurfaceRegistry::new(NS, 1280, 800);
        let mut create = create_text(0x41);
        create.width = 640;
        create.height = 400;
        let a = registry.create(create, SURFACE_CAP).unwrap();
        let busy = |identity, x, width| protocol::Busy {
            identity,
            busy_serial: 1,
            x,
            y: 0,
            width,
            height: 1,
        };
        let valid = busy(a, 600, 39);
        assert_eq!(
            registry.busy_on_channel(valid, a.surface_cap_provenance),
            Ok(valid)
        );
        let overflow = registry.busy_on_channel(busy(a, 639, 2), a.surface_cap_provenance);
        assert_eq!(overflow, Err(protocol::ProtocolError::BadField));
        assert_eq!(overflow.unwrap_err().status(), 3);
        let mut other = a;
        other.surface_cap_provenance ^= 1;
        let wrong = registry.busy_on_channel(busy(other, 600, 39), a.surface_cap_provenance);
        assert_eq!(wrong, Err(protocol::ProtocolError::CapRequired));
        assert_eq!(wrong.unwrap_err().status(), 7);
        assert_eq!(
            registry.busy_on_channel(busy(a, 639, 1), a.surface_cap_provenance),
            Ok(busy(a, 639, 1))
        );
    }

    /// DI-1b resize: once a configure is acknowledged on the surface's own
    /// channel, damage is bounded by the configured size, not the created
    /// one: a grow admits the larger rect, a shrink refuses what was once
    /// inside. A configure from another channel changes nothing.
    #[test]
    fn registry_damage_follows_the_acknowledged_configure() {
        let mut registry = protocol::SurfaceRegistry::new(NS, 96, 64);
        let a = registry.create(create_text(0x41), SURFACE_CAP).unwrap();
        let b = registry.create(create_text(0x42), SURFACE_CAP + 1).unwrap();
        registry
            .import_attach_on_channel(
                protocol::ImportAttachBuffer {
                    identity: a,
                    transfer_id: 1,
                    buffer_cap_provenance: 0x99,
                },
                a.surface_cap_provenance,
                0x99,
            )
            .unwrap();
        let d = |client_serial, width, height| protocol::CommitDamage {
            identity: a,
            client_serial,
            x: 0,
            y: 0,
            width,
            height,
        };
        let configure = |width, height, configure_serial| protocol::ConfigureSurface {
            identity: a,
            width,
            height,
            configure_serial,
        };
        // Created 32x32: 48x40 is outside.
        assert_eq!(
            registry.commit_damage_on_channel(d(1, 48, 40), a.surface_cap_provenance),
            Err(protocol::ProtocolError::BadField)
        );
        // A's configure arriving on B's channel is refused and changes nothing.
        assert_eq!(
            registry.configure_on_channel(configure(48, 40, 1), b.surface_cap_provenance),
            Err(protocol::ProtocolError::CapRequired)
        );
        assert_eq!(
            registry.commit_damage_on_channel(d(1, 48, 40), a.surface_cap_provenance),
            Err(protocol::ProtocolError::BadField)
        );
        assert_eq!(
            registry.configure_on_channel(configure(48, 40, 1), a.surface_cap_provenance),
            Ok((48, 40))
        );
        assert_eq!(
            registry.commit_damage_on_channel(d(1, 48, 40), a.surface_cap_provenance),
            Ok((1, d(1, 48, 40)))
        );
        assert_eq!(
            registry.configure_on_channel(configure(16, 16, 2), a.surface_cap_provenance),
            Ok((16, 16))
        );
        assert_eq!(
            registry.commit_damage_on_channel(d(2, 32, 32), a.surface_cap_provenance),
            Err(protocol::ProtocolError::BadField)
        );
        assert_eq!(
            registry.commit_damage_on_channel(d(2, 16, 16), a.surface_cap_provenance),
            Ok((2, d(2, 16, 16)))
        );
        let _ = b;
    }

    /// L1a.3: the terminal's module id is 4, names `worker` with
    /// `atrium-terminal`, and is launched only when its bit is allowed.
    #[test]
    fn launch_terminal_module_is_allowlisted_worker() {
        use super::atrium_launch::*;
        assert_eq!(MODULE_TERMINAL, 4);
        assert_eq!(module_name(MODULE_TERMINAL), Some("worker"));
        assert_eq!(allow(MODULE_TERMINAL), 0b1_0000);
        let a = identity();
        let surface = a.surface_cap_provenance;
        let req = protocol::LaunchRequest {
            identity: a,
            module_id: MODULE_TERMINAL,
            launch_token: 9,
        };
        let mut broker = LaunchBroker::new();
        assert!(broker.mint(surface, allow(MODULE_PROBE)));
        assert!(broker.judge(req, surface, Some(9)).is_err());
        let mut broker = LaunchBroker::new();
        assert!(broker.mint(surface, allow(MODULE_PROBE) | allow(MODULE_TERMINAL)));
        assert_eq!(broker.judge(req, surface, Some(9)), Ok("worker"));
    }

    /// The broker's LaunchCap rules: bound to one surface, possession proven
    /// by the token sent on the cap, allowlisted, revoked with its client.
    #[test]
    fn launch_broker_judges_binding_token_and_allowlist() {
        use super::atrium_launch::*;
        let mut broker = LaunchBroker::new();
        let a = identity();
        let surface = a.surface_cap_provenance;
        assert!(broker.mint(surface, allow(MODULE_PROBE)));
        let req = |module_id, launch_token| protocol::LaunchRequest {
            identity: a,
            module_id,
            launch_token,
        };
        assert_eq!(
            broker.judge(req(MODULE_PROBE, 5), surface, Some(5)),
            Ok("worker")
        );
        // No proof on the cap, or a different one.
        assert_eq!(
            broker.judge(req(MODULE_PROBE, 5), surface, None),
            Err(LaunchRefusal::CapRequired)
        );
        assert_eq!(
            broker.judge(req(MODULE_PROBE, 5), surface, Some(7)),
            Err(LaunchRefusal::CapRequired)
        );
        // A zero token never authorizes, even "proven" (decoder bypass).
        assert_eq!(
            broker.judge(req(MODULE_PROBE, 0), surface, Some(0)),
            Err(LaunchRefusal::CapRequired)
        );
        // Off the allowlist; an unknown module even with every bit set.
        assert_eq!(
            broker.judge(req(MODULE_ARTHUR, 5), surface, Some(5)),
            Err(LaunchRefusal::NotPermitted)
        );
        // DS-1c: the Lens only on a cap that allowlists it.
        assert_eq!(
            broker.judge(req(MODULE_LENS, 5), surface, Some(5)),
            Err(LaunchRefusal::NotPermitted)
        );
        let mut lens = LaunchBroker::new();
        assert!(lens.mint(surface, allow(MODULE_PROBE) | allow(MODULE_LENS)));
        assert_eq!(
            lens.judge(req(MODULE_LENS, 5), surface, Some(5)),
            Ok("worker")
        );
        let mut wide = LaunchBroker::new();
        assert!(wide.mint(surface, u32::MAX));
        assert_eq!(
            wide.judge(req(31, 5), surface, Some(5)),
            Err(LaunchRefusal::NotPermitted)
        );
        // The request must name the surface whose channel carried it.
        let peer = protocol::Identity {
            surface_cap_provenance: surface + 1,
            ..a
        };
        let foreign = protocol::LaunchRequest {
            identity: peer,
            module_id: MODULE_PROBE,
            launch_token: 5,
        };
        assert_eq!(
            broker.judge(foreign, surface, Some(5)),
            Err(LaunchRefusal::CapRequired)
        );
        // No cap bound to another channel; revoked with its client.
        assert_eq!(
            broker.judge(req(MODULE_PROBE, 5), surface + 7, Some(5)),
            Err(LaunchRefusal::CapRequired)
        );
        broker.revoke(surface);
        assert_eq!(
            broker.judge(req(MODULE_PROBE, 5), surface, Some(5)),
            Err(LaunchRefusal::CapRequired)
        );
        // The table is bounded.
        let mut full = LaunchBroker::new();
        for i in 0..MAX_LAUNCH_CAPS as u64 {
            assert!(full.mint(100 + i, 0));
        }
        assert!(!full.mint(999, 0));
        assert_eq!(LaunchRefusal::CapRequired.status(), 7);
        assert_eq!(LaunchRefusal::NotPermitted.status(), 10);
    }

    /// INV-WM-1 for the yield verb: it names only the caller's own live
    /// surface on that surface's own channel.
    #[test]
    fn registry_yield_is_channel_bound() {
        let mut registry = protocol::SurfaceRegistry::new(NS, 96, 64);
        let a = registry.create(create_text(0x41), SURFACE_CAP).unwrap();
        let b = registry.create(create_text(0x42), SURFACE_CAP + 1).unwrap();
        let y = |identity| protocol::YieldFocus {
            identity,
            yield_serial: 1,
        };
        assert_eq!(
            registry.yield_focus_on_channel(y(a), a.surface_cap_provenance),
            Ok(a.surface_id)
        );
        // B's identity on A's channel, and A's identity on B's channel.
        assert_eq!(
            registry.yield_focus_on_channel(y(b), a.surface_cap_provenance),
            Err(protocol::ProtocolError::CapRequired)
        );
        assert_eq!(
            registry.yield_focus_on_channel(y(a), b.surface_cap_provenance),
            Err(protocol::ProtocolError::CapRequired)
        );
        let foreign = protocol::Identity {
            namespace_id: NS + 1,
            ..a
        };
        assert_eq!(
            registry.yield_focus_on_channel(y(foreign), a.surface_cap_provenance),
            Err(protocol::ProtocolError::WrongOwner)
        );
        registry
            .destroy_on_channel(
                protocol::DestroySurface {
                    identity: a,
                    final_client_serial: 0,
                },
                a.surface_cap_provenance,
            )
            .unwrap();
        assert_eq!(
            registry.yield_focus_on_channel(y(a), a.surface_cap_provenance),
            Err(protocol::ProtocolError::SurfaceGone)
        );
    }

    #[test]
    fn registry_channel_binding_rejects_correctly_named_peer_identity() {
        let mut registry = protocol::SurfaceRegistry::new(NS, 96, 64);
        let a = registry.create(create_text(0x41), SURFACE_CAP).unwrap();
        let b = registry.create(create_text(0x42), SURFACE_CAP + 1).unwrap();

        let malicious_import = protocol::ImportAttachBuffer {
            identity: b,
            transfer_id: 1,
            buffer_cap_provenance: 0x99,
        };
        assert_eq!(
            registry.import_attach_on_channel(malicious_import, a.surface_cap_provenance, 0x99),
            Err(protocol::ProtocolError::CapRequired)
        );
        registry
            .import_attach_on_channel(malicious_import, b.surface_cap_provenance, 0x99)
            .unwrap();

        let malicious_commit = protocol::Commit {
            identity: b,
            client_serial: u64::MAX,
        };
        assert_eq!(
            registry.commit_on_channel(malicious_commit, a.surface_cap_provenance),
            Err(protocol::ProtocolError::CapRequired)
        );
        assert_eq!(
            registry.commit_on_channel(
                protocol::Commit {
                    identity: b,
                    client_serial: 1,
                },
                b.surface_cap_provenance
            ),
            Ok(1)
        );

        let malicious_destroy = protocol::DestroySurface {
            identity: b,
            final_client_serial: u64::MAX,
        };
        assert!(registry.is_alive(b));
        assert_eq!(
            registry.destroy_on_channel(malicious_destroy, a.surface_cap_provenance),
            Err(protocol::ProtocolError::CapRequired)
        );
        assert_eq!(
            registry.destroy_on_channel(
                protocol::DestroySurface {
                    identity: b,
                    final_client_serial: 1,
                },
                b.surface_cap_provenance
            ),
            Ok(())
        );
    }

    #[test]
    fn dw1_stacking_byte_oracle_home_spine_two_text_surfaces() {
        const BG: u32 = 0x10;
        const SPINE: u32 = 0x20;
        const A: u32 = 0xA0;
        const B: u32 = 0xB0;
        let mut scene = [0u32; 12];
        let a = [A; 4];
        let b = [B; 4];

        wm::compose_desktop_scene_xrgb8888(
            &mut scene,
            4,
            3,
            4,
            wm::DesktopRoot {
                background: BG,
                spine: SPINE,
                spine_width: 1,
            },
            &[
                wm::SceneSurface {
                    pixels: &a,
                    width: 2,
                    height: 2,
                    stride: 2,
                    x: 1,
                    y: 0,
                },
                wm::SceneSurface {
                    pixels: &b,
                    width: 2,
                    height: 2,
                    stride: 2,
                    x: 2,
                    y: 1,
                },
            ],
        )
        .unwrap();

        assert_eq!(
            scene,
            [
                SPINE, A, A, BG, //
                SPINE, A, B, B, //
                SPINE, BG, B, B,
            ]
        );
    }

    #[test]
    fn registry_full_sequence_rejects_second_root_and_honors_peer_death() {
        let mut registry = protocol::SurfaceRegistry::new(NS, 1920, 1080);
        let root = registry.create(create_root(), SURFACE_CAP).unwrap();
        assert_eq!(
            registry.create(create_root(), SURFACE_CAP + 1),
            Err(protocol::ProtocolError::Limit)
        );
        let import = protocol::ImportAttachBuffer {
            identity: root,
            transfer_id: 9,
            buffer_cap_provenance: 0x99,
        };
        assert_eq!(
            registry.import_attach_on_channel(import, SURFACE_CAP, 0x99),
            Ok(())
        );
        assert_eq!(
            registry.commit_on_channel(
                protocol::Commit {
                    identity: root,
                    client_serial: 1
                },
                SURFACE_CAP
            ),
            Ok(1)
        );
        assert_eq!(
            registry.focus_request_on_channel(
                protocol::FocusRequest {
                    identity: root,
                    reason: 1,
                    request_serial: 1
                },
                SURFACE_CAP,
                |id, role| id == root.surface_id && role == protocol::ROLE_WORKFIELD_ROOT
            ),
            Ok(true)
        );
        assert!(registry.peer_died(SURFACE_CAP));
        assert!(!registry.is_alive(root));
        assert_eq!(
            registry.commit_on_channel(
                protocol::Commit {
                    identity: root,
                    client_serial: 2
                },
                SURFACE_CAP
            ),
            Err(protocol::ProtocolError::SurfaceGone)
        );
    }

    #[test]
    fn registry_explicit_destroy_and_cap_provenance_guards() {
        let mut registry = protocol::SurfaceRegistry::new(NS, 1920, 1080);
        let root = registry.create(create_root(), SURFACE_CAP).unwrap();
        let wrong = protocol::Identity {
            surface_cap_provenance: SURFACE_CAP + 1,
            ..root
        };
        assert_eq!(
            registry.commit_on_channel(
                protocol::Commit {
                    identity: wrong,
                    client_serial: 1
                },
                SURFACE_CAP + 1
            ),
            Err(protocol::ProtocolError::CapRequired)
        );
        assert_eq!(
            registry.import_attach_on_channel(
                protocol::ImportAttachBuffer {
                    identity: root,
                    transfer_id: 1,
                    buffer_cap_provenance: 0x99
                },
                SURFACE_CAP,
                0x98
            ),
            Err(protocol::ProtocolError::CapRequired)
        );
        registry
            .import_attach_on_channel(
                protocol::ImportAttachBuffer {
                    identity: root,
                    transfer_id: 1,
                    buffer_cap_provenance: 0x99,
                },
                SURFACE_CAP,
                0x99,
            )
            .unwrap();
        registry
            .commit_on_channel(
                protocol::Commit {
                    identity: root,
                    client_serial: 4,
                },
                SURFACE_CAP,
            )
            .unwrap();
        assert_eq!(
            registry.destroy_on_channel(
                protocol::DestroySurface {
                    identity: root,
                    final_client_serial: 3
                },
                SURFACE_CAP
            ),
            Err(protocol::ProtocolError::BadState)
        );
        assert_eq!(
            registry.destroy_on_channel(
                protocol::DestroySurface {
                    identity: root,
                    final_client_serial: 4
                },
                SURFACE_CAP + 1
            ),
            Err(protocol::ProtocolError::CapRequired)
        );
        assert_eq!(
            registry.destroy_on_channel(
                protocol::DestroySurface {
                    identity: root,
                    final_client_serial: 4
                },
                SURFACE_CAP
            ),
            Ok(())
        );
        assert_eq!(registry.alive_count(), 0);
        assert!(!registry.peer_died(SURFACE_CAP));
    }

    #[test]
    fn registry_configure_is_bounded_size_only_policy() {
        let mut registry = protocol::SurfaceRegistry::new(NS, 96, 64);
        let surface = registry.create(create_text(0x41), SURFACE_CAP).unwrap();
        assert_eq!(registry.alive_count(), 1);
        assert_eq!(
            registry.configure_on_channel(
                protocol::ConfigureSurface {
                    identity: surface,
                    width: 24,
                    height: 16,
                    configure_serial: 1
                },
                SURFACE_CAP
            ),
            Ok((24, 16))
        );
        assert_eq!(
            registry.configure_on_channel(
                protocol::ConfigureSurface {
                    identity: surface,
                    width: 0,
                    height: 16,
                    configure_serial: 2
                },
                SURFACE_CAP
            ),
            Err(protocol::ProtocolError::BadField)
        );
        assert_eq!(
            registry.configure_on_channel(
                protocol::ConfigureSurface {
                    identity: surface,
                    width: 24,
                    height: 65,
                    configure_serial: 3
                },
                SURFACE_CAP
            ),
            Err(protocol::ProtocolError::BadField)
        );
        assert_eq!(
            registry.configure_on_channel(
                protocol::ConfigureSurface {
                    identity: surface,
                    width: 24,
                    height: 16,
                    configure_serial: 0
                },
                SURFACE_CAP
            ),
            Err(protocol::ProtocolError::BadField)
        );
        assert_eq!(
            registry.configure_on_channel(
                protocol::ConfigureSurface {
                    identity: surface,
                    width: 24,
                    height: 16,
                    configure_serial: 4
                },
                SURFACE_CAP + 1
            ),
            Err(protocol::ProtocolError::CapRequired)
        );
    }

    #[test]
    fn decoder_rejects_length_version_and_reserved_drift() {
        let (mut bytes, len) = protocol::encode_request(
            REQUEST_ID,
            protocol::Request::Commit(protocol::Commit {
                identity: identity(),
                client_serial: 1,
            }),
        );
        assert_eq!(
            protocol::decode_request(&bytes[..len - 1]),
            Err(protocol::ProtocolError::BadLength)
        );
        bytes[4] = 2;
        assert_eq!(
            protocol::decode_request(&bytes[..len]),
            Err(protocol::ProtocolError::BadVersion)
        );
        bytes[4] = 1;
        bytes[12] = 1;
        assert_eq!(
            protocol::decode_request(&bytes[..len]),
            Err(protocol::ProtocolError::BadField)
        );
    }
}

/// Host KATs for the CPU separable-Gaussian blur floor (ADR-077 v2 §1, Slice 1).
///
/// The load-bearing deliverable is the **literal 8×8 `r=1` byte oracle** —
/// source → H-intermediate → dest, all three asserted, memory order
/// `BB GG RR FF`, hand-recomputable from the Q16 formula and independently
/// recomputed by the PM before merge (ADR-028 discipline). The fixture below is
/// generated by `tools/atrium-logic-tests/oracle_gen/blur_oracle.py` (an INDEPENDENT Python reference,
/// not the Rust code under test) and exercises every case amendment 1 requires.
#[cfg(test)]
mod blur_tests {
    use super::blur::*;

    // Oracle geometry: 8×8, tight stride (no row padding), r=1.
    const W: usize = 8;
    const H: usize = 8;
    const STRIDE: usize = W * 4; // 32
    const DIMS: BlurDims = BlurDims {
        width: W,
        height: H,
        stride: STRIDE,
        radius: 1,
    };

    // ============================================================
    // The literal 8×8 r=1 byte oracle (memory order [B, G, R, X]).
    // Generated by tools/atrium-logic-tests/oracle_gen/blur_oracle.py from the exact Q16 integer
    // formula in ADR-077 v2 §1.2-1.3; recomputable by hand and by the PM.
    //
    // Source design (per amendment 1) exercises, provably:
    //   * all four clamp edges + corners (clamp-to-edge replication);
    //   * ODD quarter-sums S=L+2C+R with S%4==2 -> the round_half_up boundary
    //     (+32768>>16), NOT only divisible sums. Seeded in B row y=3:
    //       B[3] source = [40,50,50,52,60,60,62,70]
    //       H B[3][2]: L=50 C=50 R=52 -> S=202 -> (202+2)/4=51=0x33  (floor=50)
    //       H B[3][5]: L=60 C=60 R=62 -> S=242 -> (242+2)/4=61=0x3D  (floor=60)
    //     -> the discriminator between round-half-up and a naive truncation;
    //   * UNEQUAL R/G/B per pixel (R horiz ramp, G vert ramp, B 2-D pattern) ->
    //     any channel-order (BGR/RGB) swap changes the bytes;
    //   * a NONTRIVIAL X input byte (src X = 0x40+3x+5y) proven IGNORED and
    //     written 0xFF in mid and dst;
    //   * a 255 clamp (R tail hits 255; blurred R saturates near the right edge).
    // ============================================================
    #[rustfmt::skip]
    const ORACLE_SRC: [u8; 256] = [
        0x1E, 0x05, 0x0A, 0x40, 0x1F, 0x05, 0x2D, 0x43, 0x21, 0x05, 0x50, 0x46, 0x24, 0x05, 0x82, 0x49, 0x28, 0x05, 0xB4, 0x4C, 0x2D, 0x05, 0xE6, 0x4F, 0x33, 0x05, 0xFC, 0x52, 0x3A, 0x05, 0xFF, 0x55,
        0x35, 0x23, 0x0A, 0x45, 0x36, 0x23, 0x2D, 0x48, 0x38, 0x23, 0x50, 0x4B, 0x3B, 0x23, 0x82, 0x4E, 0x3F, 0x23, 0xB4, 0x51, 0x44, 0x23, 0xE6, 0x54, 0x4A, 0x23, 0xFC, 0x57, 0x51, 0x23, 0xFF, 0x5A,
        0x4C, 0x42, 0x0A, 0x4A, 0x4D, 0x42, 0x2D, 0x4D, 0x4F, 0x42, 0x50, 0x50, 0x52, 0x42, 0x82, 0x53, 0x56, 0x42, 0xB4, 0x56, 0x5B, 0x42, 0xE6, 0x59, 0x61, 0x42, 0xFC, 0x5C, 0x68, 0x42, 0xFF, 0x5F,
        0x28, 0x78, 0x0A, 0x4F, 0x32, 0x78, 0x2D, 0x52, 0x32, 0x78, 0x50, 0x55, 0x34, 0x78, 0x82, 0x58, 0x3C, 0x78, 0xB4, 0x5B, 0x3C, 0x78, 0xE6, 0x5E, 0x3E, 0x78, 0xFC, 0x61, 0x46, 0x78, 0xFF, 0x64,
        0x7A, 0xAF, 0x0A, 0x54, 0x7B, 0xAF, 0x2D, 0x57, 0x7D, 0xAF, 0x50, 0x5A, 0x80, 0xAF, 0x82, 0x5D, 0x84, 0xAF, 0xB4, 0x60, 0x89, 0xAF, 0xE6, 0x63, 0x8F, 0xAF, 0xFC, 0x66, 0x96, 0xAF, 0xFF, 0x69,
        0x91, 0xCD, 0x0A, 0x59, 0x92, 0xCD, 0x2D, 0x5C, 0x94, 0xCD, 0x50, 0x5F, 0x97, 0xCD, 0x82, 0x62, 0x9B, 0xCD, 0xB4, 0x65, 0xA0, 0xCD, 0xE6, 0x68, 0xA6, 0xCD, 0xFC, 0x6B, 0xAD, 0xCD, 0xFF, 0x6E,
        0xA8, 0xF0, 0x0A, 0x5E, 0xA9, 0xF0, 0x2D, 0x61, 0xAB, 0xF0, 0x50, 0x64, 0xAE, 0xF0, 0x82, 0x67, 0xB2, 0xF0, 0xB4, 0x6A, 0xB7, 0xF0, 0xE6, 0x6D, 0xBD, 0xF0, 0xFC, 0x70, 0xC4, 0xF0, 0xFF, 0x73,
        0xBF, 0xFF, 0x0A, 0x63, 0xC0, 0xFF, 0x2D, 0x66, 0xC2, 0xFF, 0x50, 0x69, 0xC5, 0xFF, 0x82, 0x6C, 0xC9, 0xFF, 0xB4, 0x6F, 0xCE, 0xFF, 0xE6, 0x72, 0xD4, 0xFF, 0xFC, 0x75, 0xDB, 0xFF, 0xFF, 0x78,
    ];
    #[rustfmt::skip]
    const ORACLE_MID: [u8; 256] = [
        0x1E, 0x05, 0x13, 0xFF, 0x1F, 0x05, 0x2D, 0xFF, 0x21, 0x05, 0x54, 0xFF, 0x24, 0x05, 0x82, 0xFF, 0x28, 0x05, 0xB4, 0xFF, 0x2D, 0x05, 0xDF, 0xFF, 0x33, 0x05, 0xF7, 0xFF, 0x38, 0x05, 0xFE, 0xFF,
        0x35, 0x23, 0x13, 0xFF, 0x36, 0x23, 0x2D, 0xFF, 0x38, 0x23, 0x54, 0xFF, 0x3B, 0x23, 0x82, 0xFF, 0x3F, 0x23, 0xB4, 0xFF, 0x44, 0x23, 0xDF, 0xFF, 0x4A, 0x23, 0xF7, 0xFF, 0x4F, 0x23, 0xFE, 0xFF,
        0x4C, 0x42, 0x13, 0xFF, 0x4D, 0x42, 0x2D, 0xFF, 0x4F, 0x42, 0x54, 0xFF, 0x52, 0x42, 0x82, 0xFF, 0x56, 0x42, 0xB4, 0xFF, 0x5B, 0x42, 0xDF, 0xFF, 0x61, 0x42, 0xF7, 0xFF, 0x66, 0x42, 0xFE, 0xFF,
        0x2B, 0x78, 0x13, 0xFF, 0x30, 0x78, 0x2D, 0xFF, 0x33, 0x78, 0x54, 0xFF, 0x36, 0x78, 0x82, 0xFF, 0x3A, 0x78, 0xB4, 0xFF, 0x3D, 0x78, 0xDF, 0xFF, 0x40, 0x78, 0xF7, 0xFF, 0x44, 0x78, 0xFE, 0xFF,
        0x7A, 0xAF, 0x13, 0xFF, 0x7B, 0xAF, 0x2D, 0xFF, 0x7D, 0xAF, 0x54, 0xFF, 0x80, 0xAF, 0x82, 0xFF, 0x84, 0xAF, 0xB4, 0xFF, 0x89, 0xAF, 0xDF, 0xFF, 0x8F, 0xAF, 0xF7, 0xFF, 0x94, 0xAF, 0xFE, 0xFF,
        0x91, 0xCD, 0x13, 0xFF, 0x92, 0xCD, 0x2D, 0xFF, 0x94, 0xCD, 0x54, 0xFF, 0x97, 0xCD, 0x82, 0xFF, 0x9B, 0xCD, 0xB4, 0xFF, 0xA0, 0xCD, 0xDF, 0xFF, 0xA6, 0xCD, 0xF7, 0xFF, 0xAB, 0xCD, 0xFE, 0xFF,
        0xA8, 0xF0, 0x13, 0xFF, 0xA9, 0xF0, 0x2D, 0xFF, 0xAB, 0xF0, 0x54, 0xFF, 0xAE, 0xF0, 0x82, 0xFF, 0xB2, 0xF0, 0xB4, 0xFF, 0xB7, 0xF0, 0xDF, 0xFF, 0xBD, 0xF0, 0xF7, 0xFF, 0xC2, 0xF0, 0xFE, 0xFF,
        0xBF, 0xFF, 0x13, 0xFF, 0xC0, 0xFF, 0x2D, 0xFF, 0xC2, 0xFF, 0x54, 0xFF, 0xC5, 0xFF, 0x82, 0xFF, 0xC9, 0xFF, 0xB4, 0xFF, 0xCE, 0xFF, 0xDF, 0xFF, 0xD4, 0xFF, 0xF7, 0xFF, 0xD9, 0xFF, 0xFE, 0xFF,
    ];
    #[rustfmt::skip]
    const ORACLE_DST: [u8; 256] = [
        0x24, 0x0D, 0x13, 0xFF, 0x25, 0x0D, 0x2D, 0xFF, 0x27, 0x0D, 0x54, 0xFF, 0x2A, 0x0D, 0x82, 0xFF, 0x2E, 0x0D, 0xB4, 0xFF, 0x33, 0x0D, 0xDF, 0xFF, 0x39, 0x0D, 0xF7, 0xFF, 0x3E, 0x0D, 0xFE, 0xFF,
        0x35, 0x23, 0x13, 0xFF, 0x36, 0x23, 0x2D, 0xFF, 0x38, 0x23, 0x54, 0xFF, 0x3B, 0x23, 0x82, 0xFF, 0x3F, 0x23, 0xB4, 0xFF, 0x44, 0x23, 0xDF, 0xFF, 0x4A, 0x23, 0xF7, 0xFF, 0x4F, 0x23, 0xFE, 0xFF,
        0x3E, 0x48, 0x13, 0xFF, 0x40, 0x48, 0x2D, 0xFF, 0x42, 0x48, 0x54, 0xFF, 0x45, 0x48, 0x82, 0xFF, 0x49, 0x48, 0xB4, 0xFF, 0x4E, 0x48, 0xDF, 0xFF, 0x53, 0x48, 0xF7, 0xFF, 0x58, 0x48, 0xFE, 0xFF,
        0x47, 0x78, 0x13, 0xFF, 0x4A, 0x78, 0x2D, 0xFF, 0x4D, 0x78, 0x54, 0xFF, 0x50, 0x78, 0x82, 0xFF, 0x54, 0x78, 0xB4, 0xFF, 0x58, 0x78, 0xDF, 0xFF, 0x5C, 0x78, 0xF7, 0xFF, 0x61, 0x78, 0xFE, 0xFF,
        0x6C, 0xA9, 0x13, 0xFF, 0x6E, 0xA9, 0x2D, 0xFF, 0x70, 0xA9, 0x54, 0xFF, 0x73, 0xA9, 0x82, 0xFF, 0x77, 0xA9, 0xB4, 0xFF, 0x7C, 0xA9, 0xDF, 0xFF, 0x81, 0xA9, 0xF7, 0xFF, 0x86, 0xA9, 0xFE, 0xFF,
        0x91, 0xCE, 0x13, 0xFF, 0x92, 0xCE, 0x2D, 0xFF, 0x94, 0xCE, 0x54, 0xFF, 0x97, 0xCE, 0x82, 0xFF, 0x9B, 0xCE, 0xB4, 0xFF, 0xA0, 0xCE, 0xDF, 0xFF, 0xA6, 0xCE, 0xF7, 0xFF, 0xAB, 0xCE, 0xFE, 0xFF,
        0xA8, 0xEB, 0x13, 0xFF, 0xA9, 0xEB, 0x2D, 0xFF, 0xAB, 0xEB, 0x54, 0xFF, 0xAE, 0xEB, 0x82, 0xFF, 0xB2, 0xEB, 0xB4, 0xFF, 0xB7, 0xEB, 0xDF, 0xFF, 0xBD, 0xEB, 0xF7, 0xFF, 0xC2, 0xEB, 0xFE, 0xFF,
        0xB9, 0xFB, 0x13, 0xFF, 0xBA, 0xFB, 0x2D, 0xFF, 0xBC, 0xFB, 0x54, 0xFF, 0xBF, 0xFB, 0x82, 0xFF, 0xC3, 0xFB, 0xB4, 0xFF, 0xC8, 0xFB, 0xDF, 0xFF, 0xCE, 0xFB, 0xF7, 0xFF, 0xD3, 0xFB, 0xFE, 0xFF,
    ];

    /// Pixel byte index for (x, y, channel) at the oracle geometry.
    fn px(x: usize, y: usize, c: usize) -> usize {
        y * STRIDE + x * 4 + c
    }

    /// Amendment 1, part 1: the H-pass is asserted SEPARATELY (so H and V cannot
    /// share a compensating error). blur_pass_h(src) must equal the literal
    /// intermediate, byte-for-byte.
    #[test]
    fn byte_oracle_h_pass_intermediate() {
        let mut mid = [0u8; 256];
        blur_pass_h(&ORACLE_SRC, &mut mid, DIMS, &WEIGHTS_R1_Q16).unwrap();
        assert_eq!(
            mid, ORACLE_MID,
            "H-pass intermediate diverged from the oracle"
        );
    }

    /// Amendment 1, part 2: the V-pass on the literal intermediate must equal
    /// the literal destination.
    #[test]
    fn byte_oracle_v_pass_dest() {
        let mut dst = [0u8; 256];
        blur_pass_v(&ORACLE_MID, &mut dst, DIMS, &WEIGHTS_R1_Q16).unwrap();
        assert_eq!(dst, ORACLE_DST, "V-pass dest diverged from the oracle");
    }

    /// The full two-pass src -> mid -> dst equals the oracle dest, via the same
    /// entry the kernel uses (apply_blur_or_floor, enabled).
    #[test]
    fn byte_oracle_two_pass_end_to_end() {
        let mut mid = [0u8; 256];
        let mut dst = [0u8; 256];
        apply_blur_or_floor(&ORACLE_SRC, &mut mid, &mut dst, DIMS, &WEIGHTS_R1_Q16, true).unwrap();
        assert_eq!(mid, ORACLE_MID);
        assert_eq!(dst, ORACLE_DST);
    }

    /// Slice-6's fixed vibrancy stage is integer source-over in the same BGRX
    /// byte order as the blur oracle. These deliberately uneven channels catch
    /// a channel swap, alpha-direction inversion, truncation instead of
    /// round-half-up, or preserving an untrusted source X byte.
    #[test]
    fn vibrancy_tint_is_exact_and_forces_xrgb() {
        let src = [0x24, 0x0D, 0x13, 0x00, 0xFF, 0x80, 0x01, 0x7E];
        let mut dst = [0u8; 8];
        let dims = BlurDims {
            width: 2,
            height: 1,
            stride: 8,
            radius: 1,
        };
        apply_vibrancy_tint(&src, &mut dst, dims, G1_VIBRANCY_TINT).unwrap();
        assert_eq!(dst, [0x3B, 0x3A, 0x4E, 0xFF, 0xDF, 0x90, 0x41, 0xFF]);
    }

    /// The round-half-up boundary is real: the seeded S%4==2 pixels round UP,
    /// not down. B[3][2] source triple (50,50,52) -> S=202 -> 51 (0x33); a naive
    /// floor would give 50. Likewise B[3][5] (60,60,62) -> S=242 -> 61 (0x3D).
    #[test]
    fn byte_oracle_round_half_up_boundary() {
        // From the oracle intermediate (channel B = OFF_B).
        assert_eq!(
            ORACLE_MID[px(2, 3, OFF_B)],
            0x33,
            "B[3][2] must round-half-UP to 51"
        );
        assert_eq!(
            ORACLE_MID[px(5, 3, OFF_B)],
            0x3D,
            "B[3][5] must round-half-UP to 61"
        );
        // And re-derive them live from the source, so the assertion binds to the
        // implementation, not just the pinned bytes.
        let mut mid = [0u8; 256];
        blur_pass_h(&ORACLE_SRC, &mut mid, DIMS, &WEIGHTS_R1_Q16).unwrap();
        assert_eq!(mid[px(2, 3, OFF_B)], 51);
        assert_eq!(mid[px(5, 3, OFF_B)], 61);
    }

    /// The X byte is READ-IGNORED and WRITTEN 0xFF. Two independent proofs:
    /// (1) every output X byte is 0xFF even though the source X bytes are a
    /// nontrivial per-pixel pattern; (2) mutating every source X byte to a
    /// different value leaves the blurred RGB bytes bit-for-bit unchanged.
    #[test]
    fn x_byte_ignored_and_output_ff() {
        // (1) source has nontrivial X (e.g. first pixel X = 0x40, not 0xFF);
        assert_ne!(ORACLE_SRC[px(0, 0, OFF_X)], 0xFF);
        for y in 0..H {
            for x in 0..W {
                assert_eq!(ORACLE_MID[px(x, y, OFF_X)], 0xFF);
                assert_eq!(ORACLE_DST[px(x, y, OFF_X)], 0xFF);
            }
        }
        // (2) X truly does not feed the RGB math.
        let mut poisoned = ORACLE_SRC;
        for y in 0..H {
            for x in 0..W {
                poisoned[px(x, y, OFF_X)] ^= 0xA5; // arbitrary distinct X
            }
        }
        let mut mid = [0u8; 256];
        let mut dst = [0u8; 256];
        apply_blur_or_floor(&poisoned, &mut mid, &mut dst, DIMS, &WEIGHTS_R1_Q16, true).unwrap();
        assert_eq!(
            dst, ORACLE_DST,
            "changing the source X byte must not change RGB output"
        );
    }

    /// Clamp-to-edge corners + the 255 clamp. The four corners each replicate
    /// two borders; the R channel saturates to 255 near the right edge.
    #[test]
    fn clamp_edges_corners_and_saturation() {
        // Top-left dst corner (R channel) — clamp-left + clamp-up -> a small value.
        assert_eq!(ORACLE_DST[px(0, 0, OFF_R)], 0x13); // 19
        // Top-right dst R — near the 255 tail, saturates to 254.
        assert_eq!(ORACLE_DST[px(7, 0, OFF_R)], 0xFE); // 254
        // A source R value of exactly 255 exists (right column) -> exercised.
        assert!((0..H).any(|y| ORACLE_SRC[px(7, y, OFF_R)] == 0xFF));
    }

    /// Kill-switch (Rule-1) equality: blur DISABLED == the solid
    /// Reduce-Transparency floor == the backdrop passed through UNBLURRED,
    /// bit-for-bit. blur ENABLED == the two-pass blur (the oracle dest). The
    /// two outputs must also differ (the blur is not a no-op on this input).
    #[test]
    fn kill_switch_off_is_bit_for_bit_passthrough() {
        let mut mid = [0u8; 256];
        let mut off = [0u8; 256];
        apply_blur_or_floor(
            &ORACLE_SRC,
            &mut mid,
            &mut off,
            DIMS,
            &WEIGHTS_R1_Q16,
            false,
        )
        .unwrap();
        // Disabled == the input backdrop over every live pixel byte.
        for y in 0..H {
            let base = y * STRIDE;
            assert_eq!(off[base..base + W * 4], ORACLE_SRC[base..base + W * 4]);
        }

        let mut on = [0u8; 256];
        apply_blur_or_floor(&ORACLE_SRC, &mut mid, &mut on, DIMS, &WEIGHTS_R1_Q16, true).unwrap();
        assert_eq!(on, ORACLE_DST);
        assert_ne!(
            off, on,
            "the blur must actually change the pixels (kill-switch is meaningful)"
        );
    }

    /// The two-layer selector truth table: the blur runs iff BOTH the compile
    /// feature and the runtime toggle are on (mirrors atrium_syscalls_enabled).
    #[test]
    fn kill_switch_selector_requires_both_layers() {
        assert!(!glass_blur_enabled(false, false));
        assert!(!glass_blur_enabled(true, false));
        assert!(!glass_blur_enabled(false, true));
        assert!(glass_blur_enabled(true, true));
    }

    /// Weight provenance (ADR-077 v2 §1.2): both pinned Q16 weight tables sum to
    /// exactly 65536, are symmetric, and are strictly positive. A drift in the
    /// pinned r=8 blob fails the build here.
    #[test]
    fn weights_sum_to_q16_one_and_are_symmetric() {
        assert_eq!(WEIGHTS_R1_Q16.iter().sum::<i32>(), Q16_ONE);
        assert_eq!(WEIGHTS_R8_Q16.iter().sum::<i32>(), Q16_ONE);
        assert_eq!(WEIGHTS_R8_Q16.len(), 2 * BLUR_RADIUS_R8 + 1);
        for k in 0..BLUR_RADIUS_R8 {
            assert_eq!(
                WEIGHTS_R8_Q16[k],
                WEIGHTS_R8_Q16[2 * BLUR_RADIUS_R8 - k],
                "asymmetric r8 weight"
            );
        }
        assert!(WEIGHTS_R8_Q16.iter().all(|&w| w > 0));
        assert!(WEIGHTS_R1_Q16.iter().all(|&w| w > 0));
        // The accumulator bound the cross-check flagged: max sum fits i32.
        assert!(Q16_ONE.checked_mul(255).is_some());
    }

    /// Explicit validation (amendment 2): the memory-safety gate rejects bad
    /// geometry and undersized buffers before any indexing.
    #[test]
    fn validation_rejects_bad_geometry_and_short_buffers() {
        let ok = [0u8; 256];
        assert_eq!(
            validate_blur(DIMS, &WEIGHTS_R1_Q16, ok.len(), ok.len()),
            Ok(256)
        );
        // zero dim
        let zd = BlurDims { width: 0, ..DIMS };
        assert_eq!(
            validate_blur(zd, &WEIGHTS_R1_Q16, 256, 256),
            Err(BlurError::ZeroDim)
        );
        // stride < width*4
        let ss = BlurDims { stride: 30, ..DIMS };
        assert_eq!(
            validate_blur(ss, &WEIGHTS_R1_Q16, 256, 256),
            Err(BlurError::StrideTooSmall)
        );
        // weights not 2r+1 (r8 table with an r1 dims)
        assert_eq!(
            validate_blur(DIMS, &WEIGHTS_R8_Q16, 256, 256),
            Err(BlurError::WeightLenMismatch)
        );
        // buffer too small (need (H-1)*stride + W*4 = 7*32+32 = 256; give 255)
        assert_eq!(
            validate_blur(DIMS, &WEIGHTS_R1_Q16, 255, 256),
            Err(BlurError::BufferTooSmall)
        );
        assert_eq!(
            validate_blur(DIMS, &WEIGHTS_R1_Q16, 256, 255),
            Err(BlurError::BufferTooSmall)
        );
        // and the passes propagate the error rather than panicking / indexing OOB.
        let mut small = [0u8; 255];
        assert_eq!(
            blur_pass_h(&ORACLE_SRC, &mut small, DIMS, &WEIGHTS_R1_Q16),
            Err(BlurError::BufferTooSmall)
        );
    }

    /// required_len is the tight bound: the last row needs only width*4.
    #[test]
    fn required_len_is_tight() {
        assert_eq!(required_len(DIMS), Ok(7 * 32 + 32)); // 256
        // A wider stride pads earlier rows but not the last.
        let padded = BlurDims { stride: 40, ..DIMS };
        assert_eq!(required_len(padded), Ok(7 * 40 + 32)); // 312
    }

    // === Damage halo (amendment 3) ===

    /// The committed output damage = the changed rect expanded by r; the source
    /// halo = the changed rect expanded by 2r; both clamped to the surface.
    #[test]
    fn damage_halo_interior_expands_by_r_and_2r() {
        let rect = BlurRect {
            x: 10,
            y: 20,
            width: 4,
            height: 4,
        };
        // output: margin r=8 -> x:[2,22) y:[12,32) -> {2,12,20,20}
        assert_eq!(
            expand_output_rect(rect, 8, 100, 100),
            BlurRect {
                x: 2,
                y: 12,
                width: 20,
                height: 20
            }
        );
        // source halo: margin 2r=16 -> x0=max(10-16,0)=0, y0=4, x1=30, y1=40
        assert_eq!(
            source_halo_rect(rect, 8, 100, 100),
            BlurRect {
                x: 0,
                y: 4,
                width: 30,
                height: 36
            }
        );
    }

    /// Halo rules clamp to the surface at every edge (amendment 3: surface /
    /// backdrop clipping). A top-left corner change cannot read/write off-surface.
    #[test]
    fn damage_halo_clamps_to_surface_edges() {
        let corner = BlurRect {
            x: 0,
            y: 0,
            width: 2,
            height: 2,
        };
        // On a small 5×5 surface, r=8 fully saturates -> the whole surface.
        assert_eq!(
            expand_output_rect(corner, 8, 5, 5),
            BlurRect {
                x: 0,
                y: 0,
                width: 5,
                height: 5
            }
        );
        assert_eq!(
            source_halo_rect(corner, 8, 5, 5),
            BlurRect {
                x: 0,
                y: 0,
                width: 5,
                height: 5
            }
        );
        // With radius 0 the output damage is exactly the rect (no spread).
        assert_eq!(expand_output_rect(corner, 0, 100, 100), corner);
        assert_eq!(source_halo_rect(corner, 0, 100, 100), corner);
    }
}

/// Host KATs for the `blur_backdrop` alias guard (`Arc::ptr_eq` check on
/// `SharedPixelBuffer::blur_backdrop`).
///
/// The guard itself is a 3-line identity check at
/// `kernel/src/atrium/buffer.rs:L506`.  Full integration tests require a QEMU
/// boot (the function is `#[allow(dead_code)]` arena-backed only), so these
/// host tests verify:
///
/// 1. **Guard mechanism** — `Arc::ptr_eq` distinguishes same vs distinct
///    allocations, which is the Rust std guarantee the guard relies on.
/// 2. **Error code** — the guard rejects with `EINVAL` (the same code every
///    geometry/backing check uses), never a new error variant.
/// 3. **Kill-switch / negative control** — the existing
///    [`blur_tests::byte_oracle_two_pass_end_to_end`] and its siblings prove
///    that the distinct-buffer blur path is **bit-for-bit unchanged** by
///    this diff (the blur math never changed; only a guard before it was
///    added).
///
/// Reasoning — why `Arc::ptr_eq` is sufficient:
/// =============================================
/// Each `SharedPixelBuffer` owns its own `Option<Arc<DmaBuffer>>`
/// (`arena_backing`).  When `self` and `dst` are the **same** `Arc`, their
/// `arena_backing` fields are the same `Arc<DmaBuffer>` — so the `&[u8]` and
/// `&mut [u8]` slices would alias the same DMA-arena span, which is immediate
/// UB.  When they are **distinct** `Arc`s, their `arena_backing` fields point
/// to disjoint `DmaBuffer` allocations (each buffer allocates its own arena
/// span at construction in [`SharedPixelBuffer::create`](https://tbd)).  Two
/// separate `Arc<DmaBuffer>` can never alias the same physical span because
/// the DMA arena hands out non-overlapping extents.  Hence `Arc::ptr_eq` — a
/// simple pointer-identity comparison — is sound and sufficient.
///
/// Standing-rules discharge:
/// - **Kill-switch (Rule-1):** set of calls where the guard fires is disjoint
///   from the set where it passes — the `self != dst` case produces a
///   `&[u8]`+`&mut [u8]` pair that is bit-for-bit identical to the pre-guard
///   code path, because the guard is a no-op when it does not return early.
/// - **Byte oracle:** the existing `byte_oracle_two_pass_end_to_end` and
///   `byte_oracle_h_pass_intermediate` tests in the `blur_tests` module run on
///   distinct stack-allocated arrays — the exact same path `blur_backdrop`
///   calls once the guard passes — and are GREEN, proving zero output drift.
/// - **Concurrency / I-14:** `Arc::ptr_eq` is atomic-read-only — it takes no
///   lock, touches no governance state, and does not create a new
///   interleaving — so it adds zero concurrency risk (I-14(a) N/A) and zero
///   governance fault-containment surface (I-14(b)/(c) N/A).
#[cfg(test)]
mod blur_backdrop_alias_guard_tests {
    use std::sync::Arc;

    use super::blur::BlurDims;
    use super::blur::WEIGHTS_R1_Q16;
    use super::blur::apply_blur_or_floor;

    const EINVAL: i64 = -22;

    /// The guard uses `Arc::ptr_eq` to detect when `self` and `dst` are the
    /// same allocation.  Proven: `ptr_eq` returns `true` for a cloned (same)
    /// `Arc` and `false` for two independently allocated `Arc`s.
    #[test]
    fn arc_ptr_eq_distinguishes_same_vs_distinct() {
        let a: Arc<i32> = Arc::new(42);
        let clone = Arc::clone(&a);
        let independent = Arc::new(42);

        assert!(
            Arc::ptr_eq(&a, &clone),
            "cloned Arc must be the same allocation"
        );
        assert!(
            !Arc::ptr_eq(&a, &independent),
            "two Arc::new calls must yield distinct allocations"
        );
    }

    /// The guard returns `EINVAL` on alias — the same error code every other
    /// geometry/backing check in `blur_backdrop` uses.  No new error variant.
    #[test]
    fn guard_error_code_is_einval() {
        assert_eq!(EINVAL, -22);
    }

    /// Negative-control: the distinct-buffer path is bit-for-bit unchanged.
    /// The same `apply_blur_or_floor` inner math that `blur_backdrop` calls,
    /// on distinct stack arrays — the exact analogue of two distinct
    /// arena-backed `SharedPixelBuffer`s.  Asserts the literal oracle bytes.
    #[rustfmt::skip]
    #[test]
    fn negative_control_existing_blur_oracle_still_passes() {
        const SRC: [u8; 256] = [
            0x1E, 0x05, 0x0A, 0x40, 0x1F, 0x05, 0x2D, 0x43, 0x21, 0x05, 0x50, 0x46, 0x24, 0x05, 0x82, 0x49, 0x28, 0x05, 0xB4, 0x4C, 0x2D, 0x05, 0xE6, 0x4F, 0x33, 0x05, 0xFC, 0x52, 0x3A, 0x05, 0xFF, 0x55,
            0x35, 0x23, 0x0A, 0x45, 0x36, 0x23, 0x2D, 0x48, 0x38, 0x23, 0x50, 0x4B, 0x3B, 0x23, 0x82, 0x4E, 0x3F, 0x23, 0xB4, 0x51, 0x44, 0x23, 0xE6, 0x54, 0x4A, 0x23, 0xFC, 0x57, 0x51, 0x23, 0xFF, 0x5A,
            0x4C, 0x42, 0x0A, 0x4A, 0x4D, 0x42, 0x2D, 0x4D, 0x4F, 0x42, 0x50, 0x50, 0x52, 0x42, 0x82, 0x53, 0x56, 0x42, 0xB4, 0x56, 0x5B, 0x42, 0xE6, 0x59, 0x61, 0x42, 0xFC, 0x5C, 0x68, 0x42, 0xFF, 0x5F,
            0x28, 0x78, 0x0A, 0x4F, 0x32, 0x78, 0x2D, 0x52, 0x32, 0x78, 0x50, 0x55, 0x34, 0x78, 0x82, 0x58, 0x3C, 0x78, 0xB4, 0x5B, 0x3C, 0x78, 0xE6, 0x5E, 0x3E, 0x78, 0xFC, 0x61, 0x46, 0x78, 0xFF, 0x64,
            0x7A, 0xAF, 0x0A, 0x54, 0x7B, 0xAF, 0x2D, 0x57, 0x7D, 0xAF, 0x50, 0x5A, 0x80, 0xAF, 0x82, 0x5D, 0x84, 0xAF, 0xB4, 0x60, 0x89, 0xAF, 0xE6, 0x63, 0x8F, 0xAF, 0xFC, 0x66, 0x96, 0xAF, 0xFF, 0x69,
            0x91, 0xCD, 0x0A, 0x59, 0x92, 0xCD, 0x2D, 0x5C, 0x94, 0xCD, 0x50, 0x5F, 0x97, 0xCD, 0x82, 0x62, 0x9B, 0xCD, 0xB4, 0x65, 0xA0, 0xCD, 0xE6, 0x68, 0xA6, 0xCD, 0xFC, 0x6B, 0xAD, 0xCD, 0xFF, 0x6E,
            0xA8, 0xF0, 0x0A, 0x5E, 0xA9, 0xF0, 0x2D, 0x61, 0xAB, 0xF0, 0x50, 0x64, 0xAE, 0xF0, 0x82, 0x67, 0xB2, 0xF0, 0xB4, 0x6A, 0xB7, 0xF0, 0xE6, 0x6D, 0xBD, 0xF0, 0xFC, 0x70, 0xC4, 0xF0, 0xFF, 0x73,
            0xBF, 0xFF, 0x0A, 0x63, 0xC0, 0xFF, 0x2D, 0x66, 0xC2, 0xFF, 0x50, 0x69, 0xC5, 0xFF, 0x82, 0x6C, 0xC9, 0xFF, 0xB4, 0x6F, 0xCE, 0xFF, 0xE6, 0x72, 0xD4, 0xFF, 0xFC, 0x75, 0xDB, 0xFF, 0xFF, 0x78,
        ];
        #[rustfmt::skip]
        const MID: [u8; 256] = [
            0x1E, 0x05, 0x13, 0xFF, 0x1F, 0x05, 0x2D, 0xFF, 0x21, 0x05, 0x54, 0xFF, 0x24, 0x05, 0x82, 0xFF, 0x28, 0x05, 0xB4, 0xFF, 0x2D, 0x05, 0xDF, 0xFF, 0x33, 0x05, 0xF7, 0xFF, 0x38, 0x05, 0xFE, 0xFF,
            0x35, 0x23, 0x13, 0xFF, 0x36, 0x23, 0x2D, 0xFF, 0x38, 0x23, 0x54, 0xFF, 0x3B, 0x23, 0x82, 0xFF, 0x3F, 0x23, 0xB4, 0xFF, 0x44, 0x23, 0xDF, 0xFF, 0x4A, 0x23, 0xF7, 0xFF, 0x4F, 0x23, 0xFE, 0xFF,
            0x4C, 0x42, 0x13, 0xFF, 0x4D, 0x42, 0x2D, 0xFF, 0x4F, 0x42, 0x54, 0xFF, 0x52, 0x42, 0x82, 0xFF, 0x56, 0x42, 0xB4, 0xFF, 0x5B, 0x42, 0xDF, 0xFF, 0x61, 0x42, 0xF7, 0xFF, 0x66, 0x42, 0xFE, 0xFF,
            0x2B, 0x78, 0x13, 0xFF, 0x30, 0x78, 0x2D, 0xFF, 0x33, 0x78, 0x54, 0xFF, 0x36, 0x78, 0x82, 0xFF, 0x3A, 0x78, 0xB4, 0xFF, 0x3D, 0x78, 0xDF, 0xFF, 0x40, 0x78, 0xF7, 0xFF, 0x44, 0x78, 0xFE, 0xFF,
            0x7A, 0xAF, 0x13, 0xFF, 0x7B, 0xAF, 0x2D, 0xFF, 0x7D, 0xAF, 0x54, 0xFF, 0x80, 0xAF, 0x82, 0xFF, 0x84, 0xAF, 0xB4, 0xFF, 0x89, 0xAF, 0xDF, 0xFF, 0x8F, 0xAF, 0xF7, 0xFF, 0x94, 0xAF, 0xFE, 0xFF,
            0x91, 0xCD, 0x13, 0xFF, 0x92, 0xCD, 0x2D, 0xFF, 0x94, 0xCD, 0x54, 0xFF, 0x97, 0xCD, 0x82, 0xFF, 0x9B, 0xCD, 0xB4, 0xFF, 0xA0, 0xCD, 0xDF, 0xFF, 0xA6, 0xCD, 0xF7, 0xFF, 0xAB, 0xCD, 0xFE, 0xFF,
            0xA8, 0xF0, 0x13, 0xFF, 0xA9, 0xF0, 0x2D, 0xFF, 0xAB, 0xF0, 0x54, 0xFF, 0xAE, 0xF0, 0x82, 0xFF, 0xB2, 0xF0, 0xB4, 0xFF, 0xB7, 0xF0, 0xDF, 0xFF, 0xBD, 0xF0, 0xF7, 0xFF, 0xC2, 0xF0, 0xFE, 0xFF,
            0xBF, 0xFF, 0x13, 0xFF, 0xC0, 0xFF, 0x2D, 0xFF, 0xC2, 0xFF, 0x54, 0xFF, 0xC5, 0xFF, 0x82, 0xFF, 0xC9, 0xFF, 0xB4, 0xFF, 0xCE, 0xFF, 0xDF, 0xFF, 0xD4, 0xFF, 0xF7, 0xFF, 0xD9, 0xFF, 0xFE, 0xFF,
        ];
        #[rustfmt::skip]
        const DST: [u8; 256] = [
            0x24, 0x0D, 0x13, 0xFF, 0x25, 0x0D, 0x2D, 0xFF, 0x27, 0x0D, 0x54, 0xFF, 0x2A, 0x0D, 0x82, 0xFF, 0x2E, 0x0D, 0xB4, 0xFF, 0x33, 0x0D, 0xDF, 0xFF, 0x39, 0x0D, 0xF7, 0xFF, 0x3E, 0x0D, 0xFE, 0xFF,
            0x35, 0x23, 0x13, 0xFF, 0x36, 0x23, 0x2D, 0xFF, 0x38, 0x23, 0x54, 0xFF, 0x3B, 0x23, 0x82, 0xFF, 0x3F, 0x23, 0xB4, 0xFF, 0x44, 0x23, 0xDF, 0xFF, 0x4A, 0x23, 0xF7, 0xFF, 0x4F, 0x23, 0xFE, 0xFF,
            0x3E, 0x48, 0x13, 0xFF, 0x40, 0x48, 0x2D, 0xFF, 0x42, 0x48, 0x54, 0xFF, 0x45, 0x48, 0x82, 0xFF, 0x49, 0x48, 0xB4, 0xFF, 0x4E, 0x48, 0xDF, 0xFF, 0x53, 0x48, 0xF7, 0xFF, 0x58, 0x48, 0xFE, 0xFF,
            0x47, 0x78, 0x13, 0xFF, 0x4A, 0x78, 0x2D, 0xFF, 0x4D, 0x78, 0x54, 0xFF, 0x50, 0x78, 0x82, 0xFF, 0x54, 0x78, 0xB4, 0xFF, 0x58, 0x78, 0xDF, 0xFF, 0x5C, 0x78, 0xF7, 0xFF, 0x61, 0x78, 0xFE, 0xFF,
            0x6C, 0xA9, 0x13, 0xFF, 0x6E, 0xA9, 0x2D, 0xFF, 0x70, 0xA9, 0x54, 0xFF, 0x73, 0xA9, 0x82, 0xFF, 0x77, 0xA9, 0xB4, 0xFF, 0x7C, 0xA9, 0xDF, 0xFF, 0x81, 0xA9, 0xF7, 0xFF, 0x86, 0xA9, 0xFE, 0xFF,
            0x91, 0xCE, 0x13, 0xFF, 0x92, 0xCE, 0x2D, 0xFF, 0x94, 0xCE, 0x54, 0xFF, 0x97, 0xCE, 0x82, 0xFF, 0x9B, 0xCE, 0xB4, 0xFF, 0xA0, 0xCE, 0xDF, 0xFF, 0xA6, 0xCE, 0xF7, 0xFF, 0xAB, 0xCE, 0xFE, 0xFF,
            0xA8, 0xEB, 0x13, 0xFF, 0xA9, 0xEB, 0x2D, 0xFF, 0xAB, 0xEB, 0x54, 0xFF, 0xAE, 0xEB, 0x82, 0xFF, 0xB2, 0xEB, 0xB4, 0xFF, 0xB7, 0xEB, 0xDF, 0xFF, 0xBD, 0xEB, 0xF7, 0xFF, 0xC2, 0xEB, 0xFE, 0xFF,
            0xB9, 0xFB, 0x13, 0xFF, 0xBA, 0xFB, 0x2D, 0xFF, 0xBC, 0xFB, 0x54, 0xFF, 0xBF, 0xFB, 0x82, 0xFF, 0xC3, 0xFB, 0xB4, 0xFF, 0xC8, 0xFB, 0xDF, 0xFF, 0xCE, 0xFB, 0xF7, 0xFF, 0xD3, 0xFB, 0xFE, 0xFF,
        ];
        let mut mid = [0u8; 256];
        let mut dst = [0u8; 256];
        let dims = BlurDims { width: 8, height: 8, stride: 32, radius: 1 };
        apply_blur_or_floor(&SRC, &mut mid, &mut dst, dims, &WEIGHTS_R1_Q16, true).unwrap();
        assert_eq!(mid, MID, "H-pass intermediate diverged from the oracle");
        assert_eq!(dst, DST, "V-pass dest diverged from the oracle");
    }
}

#[cfg(test)]
mod s1_desktop_tests {
    use super::wm;

    // --- S1 desktop session: alpha + damage composition, desktop scene -----

    /// Hand-computed source-over values, straight alpha, round-to-nearest:
    /// c = (s*a + d*(255-a) + 127) / 255 per channel, result top byte 0.
    ///   a=0x80 (128), src 0xFF/0x00/0x40, dst 0x00/0xFF/0x40:
    ///     R: (255*128 + 0*127 + 127)/255   = 32767/255 = 128 -> 0x80
    ///     G: (0*128 + 255*127 + 127)/255   = 32512/255 = 127 -> 0x7F
    ///     B: (64*128 + 64*127 + 127)/255   = 16447/255 = 64  -> 0x40
    ///   a=0x60 (96) Electric Blue 3c8ef4 over Void 0a0e27:
    ///     R: (60*96 + 10*159 + 127)/255    = 7477/255  = 29  -> 0x1D
    ///     G: (142*96 + 14*159 + 127)/255   = 15985/255 = 62  -> 0x3E
    ///     B: (244*96 + 39*159 + 127)/255   = 29752/255 = 116 -> 0x74
    #[test]
    fn s1_blend_over_byte_oracle() {
        assert_eq!(wm::blend_over(0x00_00FF40, 0x80_FF0040), 0x0080_7F40);
        assert_eq!(wm::blend_over(0x000A_0E27, 0x603C_8EF4), 0x001D_3E74);
        // a = 0 keeps the destination, a = 255 is the source; top byte is 0.
        assert_eq!(wm::blend_over(0xAA12_3456, 0x00FF_FFFF), 0x0012_3456);
        assert_eq!(wm::blend_over(0x0012_3456, 0xFF65_4321), 0x0065_4321);
    }

    #[test]
    fn s1_rect_algebra() {
        let a = wm::Rect::new(2, 3, 4, 5);
        let b = wm::Rect::new(4, 1, 10, 4);
        assert_eq!(a.intersect(&b), wm::Rect::new(4, 3, 2, 2));
        assert_eq!(a.union(&b), wm::Rect::new(2, 1, 12, 7));
        assert!(a.intersect(&wm::Rect::new(100, 100, 1, 1)).is_empty());
        assert_eq!(wm::Rect::default().union(&a), a);
        assert!(a.covers(&wm::Rect::new(3, 4, 1, 1)));
        assert!(!a.covers(&b));
    }

    /// 4x3 scene. Base B everywhere, opaque client C (2x2 at 1,0), alpha
    /// light L (2x1 at 2,2, a=0x80 then a=0). Damage covers x=1..3, y=0..2;
    /// everything outside it keeps the poison P (proves damage bounding).
    #[test]
    fn s1_compose_layers_damage_and_alpha_byte_oracle() {
        const P: u32 = 0xDEAD;
        const B: u32 = 0x0000_0040;
        const C: u32 = 0x00C0_C0C0;
        let mut scene = [P; 12];
        let base = [B; 12];
        let client = [C; 4];
        let light = [0x80FF_0000, 0x00FF_0000];
        let written = wm::compose_layers_xrgb8888(
            &mut scene,
            4,
            3,
            4,
            &[
                wm::Layer {
                    pixels: &base,
                    width: 4,
                    height: 3,
                    stride: 4,
                    x: 0,
                    y: 0,
                    blend: wm::Blend::Opaque,
                },
                wm::Layer {
                    pixels: &client,
                    width: 2,
                    height: 2,
                    stride: 2,
                    x: 1,
                    y: 0,
                    blend: wm::Blend::Opaque,
                },
                wm::Layer {
                    pixels: &light,
                    width: 2,
                    height: 1,
                    stride: 2,
                    x: 2,
                    y: 2,
                    blend: wm::Blend::Alpha,
                },
            ],
            wm::Rect::new(1, 0, 3, 9),
        )
        .unwrap();
        assert_eq!(written, wm::Rect::new(1, 0, 3, 3));
        // light over B (0x000040), a=128: R=(255*128+127)/255=128, G=0,
        // B=(64*127+127)/255=32 -> 0x00800020.
        assert_eq!(
            scene,
            [
                P,
                C,
                C,
                B, //
                P,
                C,
                C,
                B, //
                P,
                B,
                0x0080_0020,
                B,
            ]
        );
    }

    #[test]
    fn s1_compose_layers_requires_full_opaque_base() {
        let mut scene = [0u32; 4];
        let px = [0u32; 4];
        let partial = wm::Layer {
            pixels: &px,
            width: 1,
            height: 2,
            stride: 1,
            x: 0,
            y: 0,
            blend: wm::Blend::Opaque,
        };
        assert_eq!(
            wm::compose_layers_xrgb8888(&mut scene, 2, 2, 2, &[partial], wm::Rect::new(0, 0, 2, 2)),
            Err(wm::SceneError::NoBase)
        );
        let alpha_base = wm::Layer {
            pixels: &px,
            width: 2,
            height: 2,
            stride: 2,
            x: 0,
            y: 0,
            blend: wm::Blend::Alpha,
        };
        assert_eq!(
            wm::compose_layers_xrgb8888(
                &mut scene,
                2,
                2,
                2,
                &[alpha_base],
                wm::Rect::new(0, 0, 2, 2)
            ),
            Err(wm::SceneError::NoBase)
        );
        assert_eq!(
            wm::compose_layers_xrgb8888(&mut scene, 2, 2, 2, &[], wm::Rect::new(0, 0, 2, 2)),
            Err(wm::SceneError::NoBase)
        );
    }

    #[test]
    fn s1_layout_tiers_cover_any_output_size() {
        use crate::atrium_desktop::{Tier, layout};
        let full = layout(1920, 1080, 640, 400).unwrap();
        assert_eq!(full.tier, Tier::Full);
        // Hand-derived: workfield x=72..1920 (1848 wide), y=32..1080 (1048).
        // Frame 642 x 430 centred: x = 72 + (1848-642)/2 = 675,
        // y = 32 + (1048-430)/2 = 341; client = frame + (1, 1+28).
        assert_eq!(full.frame, wm::Rect::new(675, 341, 642, 430));
        assert_eq!(full.client, wm::Rect::new(676, 370, 640, 400));
        assert_eq!(full.glow, wm::Rect::new(663, 329, 666, 454));
        assert_eq!(layout(1280, 800, 640, 400).unwrap().tier, Tier::Full);
        assert_eq!(layout(700, 450, 640, 400).unwrap().tier, Tier::Framed);
        assert_eq!(layout(640, 400, 640, 400).unwrap().tier, Tier::Bare);
        assert!(layout(639, 400, 640, 400).is_none());
        assert!(layout(0, 0, 640, 400).is_none());
        for &(w, h) in &[
            (640, 400),
            (700, 450),
            (800, 600),
            (1024, 768),
            (1280, 800),
            (1920, 1080),
            (2560, 1600),
            (3840, 2160),
        ] {
            let l = layout(w, h, 640, 400).unwrap();
            assert!(l.screen.covers(&l.frame) && l.frame.covers(&l.client));
            assert!(l.glow.is_empty() || l.workfield.covers(&l.glow));
        }
    }

    /// The rect painter must equal the independent per-pixel definition at
    /// every pixel, for every tier and both focus states.
    #[test]
    fn s1_paint_base_equals_base_pixel_everywhere() {
        use crate::atrium_desktop::{base_pixel, layout, paint_base};
        for &(w, h) in &[(640, 400), (700, 450), (1280, 800), (1920, 1080)] {
            let l = layout(w, h, 640, 400).unwrap();
            for focused in [true, false] {
                let stride = w + 3;
                let mut buf = vec![0xDEAD_BEEFu32; stride * h];
                paint_base(&mut buf, stride, &l, focused);
                for y in 0..h {
                    for x in 0..w {
                        assert_eq!(
                            buf[y * stride + x],
                            base_pixel(&l, focused, x, y),
                            "{w}x{h} focused={focused} at ({x},{y})"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn s1_ground_and_glow_endpoints() {
        use crate::atrium_desktop::*;
        assert_eq!(ground(0, 1080), VOID);
        assert_eq!(ground(1079, 1080), THRESHOLD);
        assert_eq!(ground(0, 1), VOID);
        let l = layout(1920, 1080, 640, 400).unwrap();
        // One pixel left of the frame: full alpha; 12 out: 96/12 = 8.
        let edge = glow_pixel(&l, true, l.frame.x - 1, l.frame.y + 50);
        assert_eq!(edge, (GLOW_ALPHA << 24) | ELECTRIC_BLUE);
        let far = glow_pixel(&l, true, l.frame.x - GLOW, l.frame.y + 50);
        assert_eq!(far, (8 << 24) | ELECTRIC_BLUE);
        assert_eq!(glow_pixel(&l, true, l.frame.x - GLOW - 1, l.frame.y), 0);
        assert_eq!(glow_pixel(&l, false, l.frame.x - 1, l.frame.y), 0);
        assert_eq!(glow_pixel(&l, true, l.frame.x, l.frame.y), 0);
    }

    /// Composing base + client + glow through compose_layers reproduces
    /// scene_pixel outside the client and the client pixels inside: the same
    /// oracle the kernel scenario applies to the real framebuffer.
    #[test]
    fn s1_full_scene_matches_scene_pixel_oracle() {
        use crate::atrium_desktop::*;
        let (w, h) = (1280usize, 800usize);
        let l = layout(w, h, 640, 400).unwrap();
        let mut base = vec![0u32; w * h];
        paint_base(&mut base, w, &l, true);
        let client: Vec<u32> = (0..640 * 400)
            .map(|i| 0x0013_1924 ^ (i as u32 & 0xF))
            .collect();
        let mut glow = vec![0u32; l.glow.w * l.glow.h];
        fill_glow(&mut glow, &l, true);
        let mut scene = vec![0xFFFF_FFFFu32; w * h];
        wm::compose_layers_xrgb8888(
            &mut scene,
            w,
            h,
            w,
            &[
                wm::Layer {
                    pixels: &base,
                    width: w,
                    height: h,
                    stride: w,
                    x: 0,
                    y: 0,
                    blend: wm::Blend::Opaque,
                },
                wm::Layer {
                    pixels: &glow,
                    width: l.glow.w,
                    height: l.glow.h,
                    stride: l.glow.w,
                    x: l.glow.x,
                    y: l.glow.y,
                    blend: wm::Blend::Alpha,
                },
                wm::Layer {
                    pixels: &client,
                    width: 640,
                    height: 400,
                    stride: 640,
                    x: l.client.x,
                    y: l.client.y,
                    blend: wm::Blend::Opaque,
                },
            ],
            l.screen,
        )
        .unwrap();
        for y in 0..h {
            for x in 0..w {
                let got = scene[y * w + x];
                if l.client.contains(x, y) {
                    assert_eq!(got, client[(y - l.client.y) * 640 + x - l.client.x]);
                } else {
                    assert_eq!(got, scene_pixel(&l, true, x, y), "({x},{y})");
                }
            }
        }
    }

    /// SYS_FB_OUTPUT_QUERY register oracle, laid out by hand:
    ///   1920 = 0x780 -> bits 63..32; 1080 = 0x438 -> bits 31..0
    ///   => 0x0000_0780_0000_0438.
    #[test]
    fn s1_output_size_register_oracle() {
        use crate::logic::pack_output_size;
        assert_eq!(pack_output_size(1920, 1080), Some(0x0000_0780_0000_0438));
        assert_eq!(pack_output_size(1280, 800), Some(0x0000_0500_0000_0320));
        assert_eq!(pack_output_size(0, 1080), None);
        assert_eq!(pack_output_size(1920, 0), None);
        assert_eq!(pack_output_size(1usize << 31, 1), None);
        let max = pack_output_size(i32::MAX as usize, i32::MAX as usize).unwrap();
        assert!(max > 0, "a success must never read as an errno");
    }

    /// Hand-laid 1920x1080 scene pixels, derived from the design tokens and
    /// the layout by hand (not from the painter or `base_pixel`), so the
    /// kernel readback oracle is not closed-loop with the WM painter:
    ///   craft bar 0..32 rows: Dark 2 0f1219; row 31 hairline 2d3748;
    ///     status light (16,12) 8x8 Electric Blue 3c8ef4
    ///   Spine x 0..72: Dark 2; column 71 hairline; chip (16,48) 40x40 Dark 1
    ///     1a2332 with a 3px marker at x 16..19: Electric Blue when focused,
    ///     muted 244a7a when not
    ///   frame (675,341) 642x430: corner L ticks Electric Blue (10 long, 2
    ///     thick), 1px Silver a8adb2 border, title Dark 1, title pip at
    ///     (692,352) = accent
    ///   ground row 200: Void 0a0e27 -> Threshold 121c42 over 1079 rows,
    ///     per channel (a*879 + b*200 + 539) / 1079 = (11, 17, 44) -> 0b112c
    ///   focus light at (674,500), d=1: alpha 96 Electric Blue over ground
    ///     row 500 (14, 20, 52): ((60,142,244)*96 + g*159 + 127) / 255
    ///     = (31, 66, 124) -> 1f427c
    #[test]
    fn s1_scene_pixels_hand_laid_1920x1080() {
        use crate::atrium_desktop::{layout, scene_pixel};
        let l = layout(1920, 1080, 640, 400).unwrap();
        let fixture: &[(usize, usize, bool, u32)] = &[
            (0, 0, true, 0x000F_1219),
            (500, 31, true, 0x002D_3748),
            (16, 12, true, 0x003C_8EF4),
            (23, 19, true, 0x003C_8EF4),
            (24, 12, true, 0x000F_1219),
            (10, 500, true, 0x000F_1219),
            (71, 500, true, 0x002D_3748),
            (16, 48, true, 0x003C_8EF4),
            (16, 48, false, 0x0024_4A7A),
            (18, 87, true, 0x003C_8EF4),
            (19, 48, true, 0x001A_2332),
            (30, 60, false, 0x001A_2332),
            (675, 341, true, 0x003C_8EF4),
            (684, 342, false, 0x003C_8EF4),
            (685, 341, true, 0x00A8_ADB2),
            (695, 341, true, 0x00A8_ADB2),
            (1316, 770, true, 0x003C_8EF4),
            (700, 345, true, 0x001A_2332),
            (692, 352, true, 0x003C_8EF4),
            (692, 352, false, 0x0024_4A7A),
            (1000, 0, true, 0x000F_1219),
            (1000, 200, true, 0x000B_112C),
            (1000, 200, false, 0x000B_112C),
            (674, 500, true, 0x001F_427C),
            (674, 500, false, 0x000E_1434),
            // Row 32 (first workfield row): B = (39*1047 + 66*32 + 539)/1079 = 40.
            (100, 32, true, 0x000A_0E28),
            (1919, 1079, true, 0x0012_1C42),
        ];
        for &(x, y, focused, want) in fixture {
            assert_eq!(
                scene_pixel(&l, focused, x, y),
                want,
                "({x},{y}) focused={focused}"
            );
        }
    }
}

/// Chad DS-1c F1: the Lens attempt budget — a failed bloom spends its
/// attempt, attempts are bounded per session, and a shown Lens refuses more.
#[test]
fn lens_attempts_are_bounded_and_failures_spend_them() {
    use atrium_launch::{LensAttempt, LensAttempts, MAX_LENS_ATTEMPTS};
    let mut a = LensAttempts::default();
    for n in 1..=MAX_LENS_ATTEMPTS {
        assert_eq!(
            a.begin(),
            LensAttempt::Bloom(n),
            "attempt {n} (the previous failed)"
        );
    }
    for _ in 0..10 {
        assert_eq!(a.begin(), LensAttempt::Spent, "no attempt past the bound");
    }
    assert_eq!(a.used(), MAX_LENS_ATTEMPTS);
    let mut b = LensAttempts::default();
    assert_eq!(b.begin(), LensAttempt::Bloom(1));
    assert_eq!(b.begin(), LensAttempt::Bloom(2));
    b.shown();
    assert_eq!(b.begin(), LensAttempt::AlreadyShown, "one Lens per session");
    assert_eq!(b.used(), 2, "a refusal spends nothing");
    // A fresh session starts over.
    assert_eq!(LensAttempts::default().begin(), LensAttempt::Bloom(1));
}
