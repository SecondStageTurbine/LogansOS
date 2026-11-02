// SPDX-License-Identifier: MPL-2.0
//! The VT screen (THINK L1a §5): the vendored `vt100` parser and screen
//! (with `std` off), plus the terminal's own hooks: the answerback filter
//! ([`crate::answer`]) and the bell. OSC 52 clipboard reads and writes,
//! title and icon setting, and resize requests stay `vt100`'s no-ops.
//!
//! The sequences `vt100` acts on are exactly THINK §5.1's list (revision 5),
//! each with a KAT here. One local patch to the vendored copy
//! (`third_party/vt100/VENDORED.md`): `CSI <params> SP <final>` with any
//! final but `q` now reaches `unhandled_csi` with its space intermediate, so
//! the answerback filter refuses it (upstream passed no intermediates, and a
//! malformed `CSI 6 SP n` was answered as DSR 6n).

use crate::answer::answer;
use alloc::vec::Vec;
use core::num::NonZeroU16;

/// The terminal's callbacks into the parser.
#[derive(Default)]
pub struct Hooks {
    answers: Vec<u8>,
    bell: bool,
}

impl vt100::Callbacks for Hooks {
    fn audible_bell(&mut self, _: &mut vt100::Screen) {
        self.bell = true;
    }

    fn visual_bell(&mut self, _: &mut vt100::Screen) {
        self.bell = true;
    }

    fn unhandled_csi(
        &mut self,
        screen: &mut vt100::Screen,
        i1: Option<u8>,
        i2: Option<u8>,
        params: &[&[u16]],
        c: char,
    ) {
        // vte passes a private marker as the first intermediate.
        let (private, others) = match i1 {
            Some(b @ (b'?' | b'>' | b'=' | b'<')) => (Some(b), [i2, None]),
            _ => (None, [i1, i2]),
        };
        let mut intermediates = [0u8; 2];
        let mut n = 0;
        for b in others.into_iter().flatten() {
            intermediates[n] = b;
            n += 1;
        }
        // vt100 keeps a pending wrap as a cursor one past the last column;
        // a cursor report names the last column, as xterm's does.
        let (row, col) = screen.cursor_position();
        let last_col = screen.size().1.get() - 1;
        if let Ok(final_byte) = u8::try_from(c)
            && let Some(bytes) = answer(
                private,
                &intermediates[..n],
                params,
                final_byte,
                (row, col.min(last_col)),
            )
        {
            self.answers.extend_from_slice(&bytes);
        }
    }
}

/// What one batch of child output produced besides the screen update.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Output {
    /// Answerback bytes for the child's own stdin (§5.4).
    pub answers: Vec<u8>,
    /// The child rang the bell.
    pub bell: bool,
}

/// The reset every child starts from (§3.4), without clearing what the last
/// child printed: the main screen, DECCKM off, keypad numeric, bracketed
/// paste off, the cursor shown. (Autowrap is always on: `vt100` has no
/// `?7`, THINK §5.1.)
pub const CHILD_RESET: &[u8] = b"\x1b[?1049l\x1b[?1l\x1b>\x1b[?2004l\x1b[?25h";

pub struct Term {
    parser: vt100::Parser<Hooks>,
}

impl Term {
    /// A screen of `rows` × `cols` with no scrollback.
    pub fn new(rows: NonZeroU16, cols: NonZeroU16) -> Self {
        Self {
            parser: vt100::Parser::new_with_callbacks(rows, cols, 0, Hooks::default()),
        }
    }

    /// Feed child output (stdout bytes, or the discipline's echo).
    pub fn process(&mut self, bytes: &[u8]) -> Output {
        self.parser.process(bytes);
        let hooks = self.parser.callbacks_mut();
        Output {
            answers: core::mem::take(&mut hooks.answers),
            bell: core::mem::take(&mut hooks.bell),
        }
    }

    /// Reset the modes for a new child (§3.4).
    pub fn reset_for_child(&mut self) {
        self.parser.process(CHILD_RESET);
        let hooks = self.parser.callbacks_mut();
        hooks.answers.clear();
        hooks.bell = false;
    }

    pub fn screen(&self) -> &vt100::Screen {
        self.parser.screen()
    }

    /// The child's application-cursor mode, for the key encoder.
    pub fn decckm(&self) -> bool {
        self.parser.screen().application_cursor()
    }

    /// The child's bracketed-paste mode, for [`crate::paste::bracket`].
    pub fn bracketed_paste(&self) -> bool {
        self.parser.screen().bracketed_paste()
    }

    pub fn set_size(&mut self, rows: NonZeroU16, cols: NonZeroU16) {
        self.parser.set_size(rows, cols);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::String;
    use vt100::Color;

    fn term() -> Term {
        Term::new(NonZeroU16::new(3).unwrap(), NonZeroU16::new(10).unwrap())
    }

    /// The rows as THINK §5.3 writes them: 10 cells, `_` for blank.
    fn rows(t: &Term) -> [String; 3] {
        core::array::from_fn(|r| {
            (0..10)
                .map(|c| {
                    let s = t.screen().cell(r as u16, c).unwrap().contents();
                    if s.is_empty() {
                        String::from("_")
                    } else {
                        String::from(s)
                    }
                })
                .collect()
        })
    }

    const BLANK: &str = "__________";

    fn check(input: &[u8], want: [&str; 3], cursor: (u16, u16)) -> Term {
        let mut t = term();
        t.process(input);
        assert_eq!(rows(&t), want.map(String::from), "input {input:02x?}");
        assert_eq!(t.screen().cursor_position(), cursor, "input {input:02x?}");
        t
    }

    // THINK §5.3, the grid rows (a 10 × 3 screen).

    #[test]
    fn g1_cr_lf() {
        check(
            &[0x61, 0x62, 0x0d, 0x0a, 0x63],
            ["ab________", "c_________", BLANK],
            (1, 1),
        );
    }

    #[test]
    fn g2_cub_then_el0() {
        check(
            &[0x61, 0x62, 0x63, 0x1b, 0x5b, 0x32, 0x44, 0x1b, 0x5b, 0x4b],
            ["a_________", BLANK, BLANK],
            (0, 1),
        );
    }

    #[test]
    fn g3_cup_is_one_based() {
        check(
            &[0x1b, 0x5b, 0x32, 0x3b, 0x35, 0x48, 0x78],
            [BLANK, "____x_____", BLANK],
            (1, 5),
        );
    }

    /// The THINK's "(2, 9), wrap pending": vt100 keeps a pending wrap as
    /// the cursor one past the last column, `(2, 10)`; what matters is the
    /// behaviour, checked next (and DSR 6n reports column 10, below).
    #[test]
    fn g4_cup_clamps_and_wrap_is_pending() {
        let mut t = check(
            &[0x1b, 0x5b, 0x39, 0x3b, 0x39, 0x39, 0x48, 0x7a],
            [BLANK, BLANK, "_________z"],
            (2, 10),
        );
        assert_eq!(t.process(b"\x1b[6n").answers, b"\x1b[3;10R");
        // Pending: the next character wraps (and, on the last row, scrolls).
        t.process(b"Q");
        assert_eq!(
            rows(&t),
            [BLANK, "_________z", "Q_________"].map(String::from)
        );
    }

    #[test]
    fn g5_autowrap() {
        check(b"0123456789A", ["0123456789", "A_________", BLANK], (1, 1));
    }

    #[test]
    fn g6_sgr_7_then_0() {
        let t = check(
            &[0x1b, 0x5b, 0x37, 0x6d, 0x41, 0x1b, 0x5b, 0x30, 0x6d, 0x42],
            ["AB________", BLANK, BLANK],
            (0, 2),
        );
        assert!(t.screen().cell(0, 0).unwrap().inverse());
        assert!(!t.screen().cell(0, 1).unwrap().inverse());
    }

    #[test]
    fn g7_ed2_keeps_the_cursor() {
        check(
            &[0x61, 0x62, 0x1b, 0x5b, 0x32, 0x4a],
            [BLANK, BLANK, BLANK],
            (0, 2),
        );
    }

    #[test]
    fn g8_alternate_screen_round_trip() {
        let t = check(
            b"ab\x1b[?1049hcd\x1b[?1049l",
            ["ab________", BLANK, BLANK],
            (0, 2),
        );
        assert!(!t.screen().alternate_screen());
    }

    #[test]
    fn g9_decstbm_scrolls_only_the_region() {
        check(
            &[
                0x1b, 0x5b, 0x31, 0x3b, 0x32, 0x72, 0x61, 0x0a, 0x62, 0x0a, 0x63,
            ],
            ["_b________", "__c_______", BLANK],
            (1, 3),
        );
    }

    #[test]
    fn g10_kibis_size_probe_clamps() {
        let mut t = check(b"\x1b[999C\x1b[999B", [BLANK, BLANK, BLANK], (2, 9));
        // No wrap pending after a cursor move: the next character lands here.
        t.process(b"Q");
        assert_eq!(rows(&t), [BLANK, BLANK, "_________Q"].map(String::from));
    }

    #[test]
    fn g11_bare_sgr_resets() {
        let t = check(b"\x1b[7mA\x1b[mB", ["AB________", BLANK, BLANK], (0, 2));
        assert!(t.screen().cell(0, 0).unwrap().inverse());
        assert!(!t.screen().cell(0, 1).unwrap().inverse());
    }

    #[test]
    fn g12_indexed_foreground() {
        let t = check(b"\x1b[38;5;240mA", ["A_________", BLANK, BLANK], (0, 1));
        assert_eq!(t.screen().cell(0, 0).unwrap().fgcolor(), Color::Idx(240));
    }

    #[test]
    fn g13_dectcem() {
        let mut t = term();
        t.process(b"\x1b[?25l");
        assert!(t.screen().hide_cursor());
        t.process(b"\x1b[?25h");
        assert!(!t.screen().hide_cursor());
        assert_eq!(t.screen().cursor_position(), (0, 0));
    }

    // THINK §5.3, the answerback rows, through the real parser.

    #[test]
    fn answerbacks_through_the_parser() {
        let mut t = term();
        assert_eq!(t.process(b"ab\x1b[6n").answers, b"\x1b[1;3R");
        assert_eq!(t.process(b"\x1b[5n").answers, b"\x1b[0n");
        assert_eq!(t.process(b"\x1b[c").answers, b"\x1b[?1;2c");
        assert_eq!(rows(&t), ["ab________", BLANK, BLANK].map(String::from));
    }

    // THINK §5.5, the ignored forms.

    #[test]
    fn refused_reports_answer_nothing() {
        let mut t = term();
        for seq in [
            &b"\x1b[?6n"[..],
            b"\x1b[>c",
            b"\x1b[21t",
            b"\x1b]52;c;?\x07",
            b"\x1b]52;c;aGk=\x07",
            b"\x1b]0;title\x07",
        ] {
            assert_eq!(t.process(seq), Output::default(), "{seq:02x?}");
        }
        assert_eq!(rows(&t), [BLANK, BLANK, BLANK].map(String::from));
    }

    #[test]
    fn a_sequence_split_across_writes_draws_the_same() {
        let mut one = term();
        one.process(b"\x1b[2;5Hx");
        let mut two = term();
        two.process(b"\x1b[2;");
        two.process(b"5Hx");
        assert_eq!(rows(&one), rows(&two));
        assert_eq!(
            one.screen().cursor_position(),
            two.screen().cursor_position()
        );
    }

    #[test]
    fn unknown_and_oversized_sequences_change_nothing() {
        let mut t = term();
        t.process(b"\x1b[5y");
        let mut many = Vec::from(&b"\x1b["[..]);
        for i in 0..40 {
            if i > 0 {
                many.push(b';');
            }
            many.push(b'1');
        }
        many.push(b'y');
        t.process(&many);
        assert_eq!(rows(&t), [BLANK, BLANK, BLANK].map(String::from));
        assert_eq!(t.screen().cursor_position(), (0, 0));
    }

    /// vt100 hands U+FFFD and C1 code points to `unhandled_char`, which the
    /// terminal ignores: invalid UTF-8 and C1 draw nothing (THINK §5.5,
    /// revision 4).
    #[test]
    fn invalid_utf8_and_c1_draw_nothing() {
        for input in [
            &[0xffu8, b'a'][..],
            &[0xc3, 0x28],
            &[0xe2, 0x82, b'a'],
            &[0x80, b'a'],
            &[0xc0, 0x80, b'a'],
            "\u{9b}a".as_bytes(),
        ] {
            let mut t = term();
            t.process(input);
            let want = if input == [0xc3, 0x28] { "(" } else { "a" };
            assert_eq!(
                t.screen().cell(0, 0).unwrap().contents(),
                want,
                "{input:02x?}"
            );
            assert_eq!(
                t.screen().cell(0, 1).unwrap().contents(),
                "",
                "{input:02x?}"
            );
        }
    }

    #[test]
    fn bel_rings() {
        let mut t = term();
        assert!(t.process(b"\x07").bell);
        assert!(!t.process(b"x").bell);
    }

    /// §3.4: a child that left raw-ish modes behind can't break the next one.
    #[test]
    fn reset_for_child_restores_the_modes() {
        let mut t = term();
        t.process(b"kept\x1b[?1h\x1b[?2004h\x1b[?25l\x1b[?7l\x1b[?1049h");
        assert!(t.decckm() && t.bracketed_paste() && t.screen().alternate_screen());
        t.reset_for_child();
        assert!(!t.decckm());
        assert!(!t.bracketed_paste());
        assert!(!t.screen().hide_cursor());
        assert!(!t.screen().alternate_screen());
        // The main screen's text is still there.
        assert_eq!(rows(&t)[0], "kept______");
        // Autowrap stayed on: `?7l` is not in the subset and is ignored.
        t.process(b"\r0123456789A");
        assert_eq!(rows(&t)[1], "A_________");
    }

    // THINK §5.1 revision 5: what vt100 also acts on, each pinned.

    #[test]
    fn hpa_is_cha() {
        check(b"\x1b[4`x", ["___x______", BLANK, BLANK], (0, 4));
    }

    #[test]
    fn rep_repeats_the_last_printed_character() {
        check(b"a\x1b[3b", ["aaaa______", BLANK, BLANK], (0, 4));
        // Nothing printed yet: nothing to repeat.
        check(b"\x1b[3b", [BLANK, BLANK, BLANK], (0, 0));
        // A huge count is bounded by the screen: 1 + 65535 = 65536 = 6553
        // full rows of 10 and 6 more, so the last screenful is two full rows
        // and 6 cells.
        check(
            b"z\x1b[65535b",
            ["zzzzzzzzzz", "zzzzzzzzzz", "zzzzzz____"],
            (2, 6),
        );
    }

    #[test]
    fn vt_and_ff_are_line_feeds() {
        check(
            b"a\x0bb\x0cc",
            ["a_________", "_b________", "__c_______"],
            (2, 3),
        );
    }

    #[test]
    fn decscusr_sets_only_the_cursor_style() {
        let mut t = term();
        assert_eq!(t.process(b"\x1b[4 q"), Output::default());
        assert_eq!(
            t.screen().cursor_style(),
            vt100::CursorStyle::SteadyUnderline
        );
        assert_eq!(rows(&t), [BLANK, BLANK, BLANK].map(String::from));
    }

    #[test]
    fn decsed_and_decsel_erase_like_ed_and_el() {
        check(b"abc\x1b[2D\x1b[?K", ["a_________", BLANK, BLANK], (0, 1));
        check(b"ab\x1b[?2J", [BLANK, BLANK, BLANK], (0, 2));
    }

    #[test]
    fn esc_g_is_the_visual_bell() {
        assert!(term().process(b"\x1bg").bell);
    }

    #[test]
    fn mouse_modes_are_state_only() {
        let mut t = term();
        for seq in [
            &b"\x1b[?1000h"[..],
            b"\x1b[?1002h",
            b"\x1b[?1003h",
            b"\x1b[?1006h",
            b"\x1b[?9h",
        ] {
            assert_eq!(t.process(seq), Output::default(), "{seq:02x?}");
        }
        assert_ne!(
            t.screen().mouse_protocol_mode(),
            vt100::MouseProtocolMode::None
        );
        t.reset_for_child();
        assert_eq!(rows(&t), [BLANK, BLANK, BLANK].map(String::from));
    }

    #[test]
    fn origin_mode_and_mode_47() {
        // ?6: CUP is relative to the scroll region.
        check(
            b"\x1b[2;3r\x1b[?6h\x1b[1;1Hx",
            [BLANK, "x_________", BLANK],
            (1, 1),
        );
        // ?47: the alternate screen without saving the cursor. vt100 keeps
        // a cursor per screen, so leaving it restores the main screen's
        // cursor (xterm shares one cursor; THINK §5.1 records the
        // divergence; kibi uses 1049).
        let t = check(
            b"ab\x1b[?47hcd\x1b[?47l",
            ["ab________", BLANK, BLANK],
            (0, 2),
        );
        assert!(!t.screen().alternate_screen());
    }

    #[test]
    fn autowrap_cannot_be_turned_off() {
        check(
            b"\x1b[?7l0123456789A",
            ["0123456789", "A_________", BLANK],
            (1, 1),
        );
    }

    /// Jarvis #42 item 1: `CSI 6 SP n` is not DSR 6n (the vendored patch).
    #[test]
    fn a_space_intermediate_is_not_dsr() {
        let mut t = term();
        assert_eq!(t.process(b"ab\x1b[6 n"), Output::default());
        assert_eq!(t.process(b"\x1b[5 n"), Output::default());
        assert_eq!(t.process(b"\x1b[ c"), Output::default());
        assert_eq!(rows(&t), ["ab________", BLANK, BLANK].map(String::from));
        // The real DSR still answers.
        assert_eq!(t.process(b"\x1b[6n").answers, b"\x1b[1;3R");
    }
}
