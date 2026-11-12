// SPDX-License-Identifier: MPL-2.0
//! Host KATs for `kernel/src/persist_logic.rs` (OT-100 R3,
//! `docs/THINK-2026-11-10-ot100-usb-persistence.md` §2.3, §4). The module is
//! mounted by `#[path]`, so these run the exact code the kernel does.

#![allow(dead_code)]

#[path = "../../../kernel/src/persist_logic.rs"]
mod persist_logic;

#[cfg(test)]
mod cmdline_tests {
    use super::persist_logic::*;

    fn p(line: &[u8]) -> Persist {
        parse_cmdline(Some(line))
    }

    #[test]
    fn only_the_exact_token_turns_it_on() {
        assert_eq!(p(b"persist=on"), Persist::On);
        assert_eq!(p(b"  persist=on\t"), Persist::On);
        assert_eq!(p(b"quiet persist=on kaslr=1"), Persist::On);
    }

    #[test]
    fn off_is_the_default() {
        assert_eq!(parse_cmdline(None), Persist::Off, "no response");
        assert_eq!(p(b""), Persist::Off, "empty");
        assert_eq!(p(b"quiet"), Persist::Off, "no token");
        assert_eq!(p(b"persist=off"), Persist::Off);
    }

    #[test]
    fn anything_else_is_off() {
        for line in [
            &b"persist=ON"[..],
            b"persist=1",
            b"persist=on,",
            b"persist=onx",
            b"persist=",
            b"persist",
            b"PERSIST=on",
            b"xpersist=on",
            b"persist =on",
            b"persist=o\xffn",
        ] {
            assert_eq!(p(line), Persist::Off, "{line:?}");
        }
    }

    #[test]
    fn a_duplicate_is_off() {
        assert_eq!(p(b"persist=on persist=on"), Persist::Off);
        assert_eq!(p(b"persist=on persist=off"), Persist::Off);
        assert_eq!(p(b"persist=off persist=on"), Persist::Off);
    }

    #[test]
    fn non_utf8_elsewhere_doesnt_matter() {
        assert_eq!(p(b"\xff\xfe persist=on \x80"), Persist::On);
    }

    #[test]
    fn the_c_string_must_end_within_reach() {
        assert_eq!(c_string(b"persist=on\0junk"), Some(&b"persist=on"[..]));
        assert_eq!(c_string(b"\0"), Some(&b""[..]));
        assert_eq!(c_string(b"no terminator"), None);
        let mut long = vec![b'a'; CMDLINE_MAX];
        long.push(0);
        assert_eq!(c_string(&long), None, "the NUL is one past the reach");
        long[CMDLINE_MAX - 1] = 0;
        assert_eq!(c_string(&long).map(<[u8]>::len), Some(CMDLINE_MAX - 1));
    }
}

#[cfg(test)]
mod mount_tests {
    use super::persist_logic::*;

    const A: DiskGuid = [0xa1; 16];
    const B: DiskGuid = [0xb2; 16];

    #[test]
    fn the_boot_device_is_identified_only_without_guessing() {
        let devs = [(0, Some(B)), (1, Some(A)), (2, None)];
        assert_eq!(identify(Some(A), &devs), BootDevice::Identified(1));
        // No GUID, or the all-zero GUID of a medium without a GPT.
        assert_eq!(
            identify(None, &devs),
            BootDevice::NotIdentified {
                guid_known: false,
                matches: 0
            }
        );
        assert_eq!(
            identify(Some([0; 16]), &[(0, Some([0; 16]))]),
            BootDevice::NotIdentified {
                guid_known: false,
                matches: 0
            }
        );
        // No device carries it.
        assert_eq!(
            identify(Some(A), &[(0, Some(B)), (1, None)]),
            BootDevice::NotIdentified {
                guid_known: true,
                matches: 0
            }
        );
        // Two sticks written from one image.
        assert_eq!(
            identify(Some(A), &[(0, Some(A)), (1, Some(A))]),
            BootDevice::NotIdentified {
                guid_known: true,
                matches: 2
            }
        );
    }

    fn rw() -> RwFacts {
        RwFacts {
            usb: true,
            boot: BootDevice::Identified(3),
            device: 3,
            persist: Persist::On,
            armed_exactly_the_partition: true,
            write_protected: Some(false),
            sync_ok: true,
        }
    }

    #[test]
    fn the_boot_stick_with_everything_in_place_is_read_write() {
        assert_eq!(rw_decision(&rw()), Ok(()));
        // An unknown write-protect state (the probe's MODE SENSE failed) is
        // still read-write, as THINK §4.1 says.
        assert_eq!(
            rw_decision(&RwFacts {
                write_protected: None,
                ..rw()
            }),
            Ok(())
        );
    }

    #[test]
    fn each_condition_refuses_alone_and_in_order() {
        let cases = [
            (
                RwFacts {
                    boot: BootDevice::NotIdentified {
                        guid_known: true,
                        matches: 2,
                    },
                    ..rw()
                },
                RwRefusal::BootNotIdentified,
            ),
            (RwFacts { device: 4, ..rw() }, RwRefusal::NotTheBootDevice),
            (
                RwFacts {
                    persist: Persist::Off,
                    ..rw()
                },
                RwRefusal::PersistOff,
            ),
            (
                RwFacts {
                    armed_exactly_the_partition: false,
                    ..rw()
                },
                RwRefusal::ArmingMismatch,
            ),
            (
                RwFacts {
                    write_protected: Some(true),
                    ..rw()
                },
                RwRefusal::WriteProtected,
            ),
            (
                RwFacts {
                    sync_ok: false,
                    ..rw()
                },
                RwRefusal::NoDurableFlush,
            ),
        ];
        for (facts, refusal) in cases {
            assert_eq!(rw_decision(&facts), Err(refusal), "{facts:?}");
        }
        // Several failing: the first in the rule's order is named.
        let all_bad = RwFacts {
            device: 4,
            persist: Persist::Off,
            sync_ok: false,
            ..rw()
        };
        assert_eq!(rw_decision(&all_bad), Err(RwRefusal::NotTheBootDevice));
    }

    /// A volume on an internal disk keeps today's selection, whatever the
    /// stick facts say: an installed LogansOS still mounts its own volume.
    #[test]
    fn a_non_usb_volume_keeps_todays_selection() {
        let internal = RwFacts {
            usb: false,
            boot: BootDevice::NotIdentified {
                guid_known: false,
                matches: 0,
            },
            persist: Persist::Off,
            armed_exactly_the_partition: false,
            sync_ok: false,
            ..rw()
        };
        assert_eq!(rw_decision(&internal), Ok(()));
    }

    /// THINK §4.2's classifier: DATA PROTECT, a pulled stick, a timeout and a
    /// failed flush each latch their reason; the codes round-trip.
    #[test]
    fn the_classifier() {
        assert_eq!(classify(BlockFailure::WriteRefused), Lost::WriteProtected);
        assert_eq!(classify(BlockFailure::NoDevice), Lost::Removed);
        assert_eq!(classify(BlockFailure::Io), Lost::NotResponding);
        assert_eq!(classify(BlockFailure::Flush), Lost::FlushFailed);
        assert_eq!(classify(BlockFailure::Other), Lost::Other);
        for lost in [
            Lost::WriteProtected,
            Lost::Removed,
            Lost::NotResponding,
            Lost::FlushFailed,
            Lost::User,
            Lost::Other,
        ] {
            assert_ne!(lost.code(), 0, "0 is not lost");
            assert_eq!(Lost::from_code(lost.code()), Some(lost));
            assert!(!lost.reason().is_empty() && !lost.notice().is_empty());
        }
        assert_eq!(Lost::from_code(0), None);
        assert_eq!(Lost::from_code(7), None);
    }
}
