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
