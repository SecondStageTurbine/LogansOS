// SPDX-License-Identifier: MPL-2.0
//! E1's oracles (THINK §7, the PM's relay for E1):
//! - the gap buffer's insert and delete at every boundary, against `String`;
//! - the line index after edits, against a fresh scan;
//! - undo and redo up to the 1 MiB bound and past it;
//! - cursor stops against Unicode 17.0.0's raw `GraphemeBreakTest.txt`;
//! - the UTF-8 / no-NUL text test and the byte-exact format round trips.

use alloc::string::{String, ToString};
use alloc::vec;
use alloc::vec::Vec;

use crate::cursor::{self, Selection};
use crate::document::{Document, Motion};
use crate::format::{self, LineEnding, NotText};
use crate::gap::GapBuffer;
use crate::history::{HISTORY_BYTES, History};
use crate::lines::LineIndex;
use crate::wrap::{self, FixedAdvance, WrapCache};

/// Text with 1-, 2-, 3- and 4-byte chars, a combining mark and line ends.
const MIXED: &str = "a\u{e9}\u{20ac}\u{1f600}\nb\u{301}c\n\nz";

fn boundaries(s: &str) -> Vec<usize> {
    (0..=s.len()).filter(|&i| s.is_char_boundary(i)).collect()
}

/// A gap buffer holding `text` with its gap at `gap`.
fn gapped(text: &str, gap: usize) -> GapBuffer {
    let mut b = GapBuffer::from_text(text);
    b.insert(gap, "");
    assert_eq!(b.gap_at(), gap);
    b
}

fn check_buffer(b: &GapBuffer, model: &str) {
    assert_eq!(b.len(), model.len());
    assert_eq!(b.text(), model);
    let (front, back) = b.halves();
    assert_eq!(alloc::format!("{front}{back}"), model);
    for i in 0..=model.len() {
        assert_eq!(
            b.is_char_boundary(i),
            model.is_char_boundary(i),
            "boundary {i}"
        );
        if i < model.len() {
            assert_eq!(b.byte(i), model.as_bytes()[i]);
        }
    }
}

#[test]
fn gap_insert_at_every_boundary_with_the_gap_at_every_boundary() {
    for &gap in &boundaries(MIXED) {
        for &at in &boundaries(MIXED) {
            for ins in ["", "X", "\u{20ac}\n", "\u{1f600}\u{1f600}"] {
                let mut b = gapped(MIXED, gap);
                b.insert(at, ins);
                let mut model = MIXED.to_string();
                model.insert_str(at, ins);
                check_buffer(&b, &model);
            }
        }
    }
}

#[test]
fn gap_delete_every_range_with_the_gap_at_every_boundary() {
    let bs = boundaries(MIXED);
    for &gap in &bs {
        for (i, &start) in bs.iter().enumerate() {
            for &end in &bs[i..] {
                let mut b = gapped(MIXED, gap);
                let removed = b.delete(start..end);
                assert_eq!(removed, &MIXED[start..end]);
                let mut model = MIXED.to_string();
                model.replace_range(start..end, "");
                check_buffer(&b, &model);
                assert_eq!(b.slice(0..model.len()), model);
            }
        }
    }
}

#[test]
fn gap_grows_past_its_first_allocation() {
    let mut b = GapBuffer::new();
    let mut model = String::new();
    for i in 0..2000 {
        let piece = if i % 3 == 0 { "\u{20ac}" } else { "ab" };
        let at = model.len() / 2;
        let at = (0..=at).rev().find(|&a| model.is_char_boundary(a)).unwrap();
        b.insert(at, piece);
        model.insert_str(at, piece);
    }
    check_buffer(&b, &model);
}

#[test]
#[should_panic(expected = "insert off a char boundary")]
fn gap_refuses_an_insert_inside_a_char() {
    GapBuffer::from_text("\u{20ac}").insert(1, "x");
}

/// A small deterministic generator (xorshift64*), so failures replay.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn pick<'a>(&mut self, xs: &[&'a str]) -> &'a str {
        xs[self.below(xs.len())]
    }
}

fn random_boundary(rng: &mut Rng, s: &str) -> usize {
    let bs = boundaries(s);
    bs[rng.below(bs.len())]
}

const PIECES: &[&str] = &[
    "a",
    "\n",
    "\u{e9}",
    "\u{20ac}\n\n",
    "xy\nz",
    "\u{1f600}",
    "e\u{301}",
    " ",
    "\t",
];

#[test]
fn line_index_matches_a_fresh_scan_after_every_edit() {
    let mut rng = Rng(0x0E1_11E5);
    let mut buf = GapBuffer::new();
    let mut lines = LineIndex::default();
    let mut model = String::new();
    for _ in 0..4000 {
        if model.is_empty() || rng.below(3) != 0 {
            let at = random_boundary(&mut rng, &model);
            let piece = rng.pick(PIECES);
            buf.insert(at, piece);
            lines.insert(at, piece);
            model.insert_str(at, piece);
        } else {
            let a = random_boundary(&mut rng, &model);
            let b = random_boundary(&mut rng, &model);
            let (a, b) = (a.min(b), a.max(b));
            buf.delete(a..b);
            lines.delete(a..b);
            model.replace_range(a..b, "");
        }
        assert_eq!(lines, LineIndex::scan(&model), "after an edit to {model:?}");
        assert_eq!(buf.text(), model);
    }
}

#[test]
fn line_index_hand_cases() {
    let idx = LineIndex::scan("ab\ncd\n");
    assert_eq!((idx.count(), idx.start(1), idx.start(2)), (3, 3, 6));
    assert_eq!(
        (
            idx.line_of(0),
            idx.line_of(2),
            idx.line_of(3),
            idx.line_of(6)
        ),
        (0, 0, 1, 2)
    );
    let mut idx = idx;
    idx.delete(2..4); // "ab" + "d\n": the first line end is gone
    assert_eq!(idx, LineIndex::scan("abd\n"));
    idx.insert(0, "\n\n");
    assert_eq!(idx, LineIndex::scan("\n\nabd\n"));
}

/// Random document operations; after each, the index, the selection and
/// the text are consistent, and undo all / redo all replay exactly.
#[test]
fn document_edits_keep_everything_consistent_and_undo_exactly() {
    for seed in 1..=8u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let mut doc = Document::new();
        let mut states = vec![doc.text()];
        for _ in 0..600 {
            match rng.below(8) {
                0..=2 => {
                    doc.type_text(rng.pick(PIECES));
                }
                3 => {
                    doc.backspace();
                }
                4 => {
                    doc.delete_forward();
                }
                5 => {
                    let at = rng.below(doc.len() + 1);
                    doc.set_head(at, rng.below(2) == 0);
                }
                6 => {
                    doc.paste("pasted\r\nline");
                }
                _ => {
                    let m = [
                        Motion::Left,
                        Motion::Right,
                        Motion::WordLeft,
                        Motion::WordRight,
                        Motion::LineStart,
                        Motion::LineEnd,
                    ];
                    doc.motion(m[rng.below(m.len())], rng.below(2) == 0);
                }
            }
            let text = doc.text();
            assert_eq!(*doc.lines(), LineIndex::scan(&text));
            let sel = doc.selection();
            assert!(
                sel.end() <= text.len()
                    && text.is_char_boundary(sel.anchor)
                    && text.is_char_boundary(sel.head)
            );
            assert!(!text.contains("\r\n"), "paste must store LF line ends");
            if states.last() != Some(&text) {
                states.push(text);
            }
        }
        let last = doc.text();
        while doc.undo().is_some() {}
        assert_eq!(doc.text(), "", "undo all, seed {seed}");
        assert!(!doc.is_dirty());
        while doc.redo().is_some() {}
        assert_eq!(doc.text(), last, "redo all, seed {seed}");
        assert_eq!(*doc.lines(), LineIndex::scan(&last));
    }
}

#[test]
fn typing_coalesces_by_word_and_backspace_by_run() {
    let mut doc = Document::new();
    for c in "hello world".chars() {
        doc.type_text(c.encode_utf8(&mut [0; 4]));
    }
    assert_eq!(doc.history().undo_depth(), 2, "\"hello \" and \"world\"");
    doc.backspace();
    doc.backspace();
    assert_eq!(doc.text(), "hello wor");
    assert_eq!(doc.history().undo_depth(), 3, "one backspace run");
    doc.undo();
    assert_eq!(doc.text(), "hello world");
    doc.undo();
    assert_eq!(doc.text(), "hello ");
    doc.redo();
    doc.redo();
    assert_eq!(doc.text(), "hello wor");
    // A move ends the run; so does a save.
    doc.type_text("l");
    doc.motion(Motion::Left, false);
    doc.motion(Motion::Right, false);
    doc.type_text("d");
    assert_eq!(doc.history().undo_depth(), 5);
    doc.mark_saved();
    doc.type_text("!");
    assert!(doc.is_dirty());
    doc.undo();
    assert!(!doc.is_dirty(), "back at the saved state");
    assert_eq!(doc.text(), "hello world");
}

#[test]
fn undo_up_to_the_bound_keeps_every_edit() {
    const CHUNK: usize = 1000;
    let chunk = "x".repeat(CHUNK);
    let fits = HISTORY_BYTES / CHUNK;
    let mut doc = Document::new();
    for _ in 0..fits {
        doc.paste(&chunk);
    }
    let h = doc.history();
    assert_eq!(
        (h.undo_depth(), h.bytes(), h.trimmed()),
        (fits, fits * CHUNK, false)
    );
    assert!(h.bytes() <= HISTORY_BYTES);
    while doc.undo().is_some() {}
    assert_eq!(doc.text(), "");
    assert!(
        !doc.is_dirty(),
        "every edit was kept, so undo reaches the opened text"
    );
}

#[test]
fn undo_past_the_bound_drops_the_oldest_and_says_so() {
    const CHUNK: usize = 1000;
    let fits = HISTORY_BYTES / CHUNK;
    let extra = 37;
    let mut doc = Document::new();
    for i in 0..fits + extra {
        let mark = char::from(b'a' + (i % 26) as u8);
        doc.paste(&mark.to_string().repeat(CHUNK));
    }
    let full = doc.text();
    let h = doc.history();
    assert_eq!((h.undo_depth(), h.trimmed()), (fits, true));
    assert!(h.bytes() <= HISTORY_BYTES && h.bytes() == fits * CHUNK);
    while doc.undo().is_some() {}
    assert_eq!(
        doc.text(),
        full[..extra * CHUNK],
        "the oldest {extra} edits are gone"
    );
    assert!(doc.is_dirty(), "the opened text can't be reached any more");
    while doc.redo().is_some() {}
    assert_eq!(doc.text(), full);
}

#[test]
fn a_single_edit_bigger_than_the_bound_is_not_kept() {
    let mut h = History::with_cap(10);
    assert!(h.undo().is_none() && h.redo().is_none());
    let mut doc = Document::new();
    doc.paste(&"y".repeat(HISTORY_BYTES + 1));
    assert_eq!((doc.history().undo_depth(), doc.history().bytes()), (0, 0));
    assert!(doc.history().trimmed());
}

#[test]
fn a_new_edit_after_undo_drops_the_redo_tail() {
    let mut doc = Document::new();
    doc.paste("one");
    doc.paste(" two");
    doc.undo();
    doc.paste(" three");
    assert_eq!(doc.history().redo_depth(), 0);
    assert!(doc.redo().is_none());
    assert_eq!(doc.text(), "one three");
}

// ---- Unicode 17.0.0 GraphemeBreakTest.txt (raw, vendored) ----

const GRAPHEME_BREAK_TEST: &str =
    include_str!("../../../third_party/unicode-ucd-17/GraphemeBreakTest.txt");

/// FNV-1a over the file's bytes, a pin that its content is the vendored
/// one (its SHA-256 is in `third_party/unicode-ucd-17/VENDORED.md`).
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, &b| {
        (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)
    })
}

/// Each test line as (the string, the byte offsets of its breaks).
fn grapheme_cases() -> Vec<(String, Vec<usize>)> {
    let mut cases = Vec::new();
    for line in GRAPHEME_BREAK_TEST.lines() {
        let data = line.split('#').next().unwrap().trim();
        if data.is_empty() {
            continue;
        }
        let mut text = String::new();
        let mut breaks = Vec::new();
        for token in data.split_whitespace() {
            match token {
                "\u{f7}" => breaks.push(text.len()),
                "\u{d7}" => {}
                hex => text.push(char::from_u32(u32::from_str_radix(hex, 16).unwrap()).unwrap()),
            }
        }
        cases.push((text, breaks));
    }
    cases
}

#[test]
fn the_vendored_test_is_unicode_17_and_matches_the_dependency() {
    assert!(GRAPHEME_BREAK_TEST.starts_with("# GraphemeBreakTest-17.0.0.txt\n"));
    assert_eq!(GRAPHEME_BREAK_TEST.len(), GRAPHEME_BREAK_TEST_LEN);
    assert_eq!(
        fnv1a(GRAPHEME_BREAK_TEST.as_bytes()),
        GRAPHEME_BREAK_TEST_FNV1A
    );
    // A bump of `unicode-segmentation` to another Unicode version must bring
    // that version's test file with it.
    assert_eq!(unicode_segmentation::UNICODE_VERSION, (17, 0, 0));
    assert_eq!(grapheme_cases().len(), 766);
}

const GRAPHEME_BREAK_TEST_LEN: usize = 126_570;
const GRAPHEME_BREAK_TEST_FNV1A: u64 = 0x6133_1052_ea9e_1ebc;

#[test]
fn cursor_stops_are_the_unicode_grapheme_breaks() {
    for (text, breaks) in grapheme_cases() {
        let mut forward = vec![0];
        let mut at = 0;
        while let Some(next) = cursor::next_grapheme(&text, at) {
            forward.push(next);
            at = next;
        }
        assert_eq!(forward, breaks, "next_grapheme over {text:?}");
        let mut backward = vec![text.len()];
        let mut at = text.len();
        while let Some(prev) = cursor::prev_grapheme(&text, at) {
            backward.push(prev);
            at = prev;
        }
        backward.reverse();
        assert_eq!(backward, breaks, "prev_grapheme over {text:?}");
    }
}

/// The same breaks through the document's moves, which work one line at a
/// time over the gap buffer, with the gap at the start, middle and end.
#[test]
fn document_moves_stop_on_the_unicode_grapheme_breaks() {
    for (text, breaks) in grapheme_cases() {
        let mids = boundaries(&text);
        for gap in [0, mids[mids.len() / 2], text.len()] {
            let mut doc = Document::new();
            doc.apply(0, 0, &text);
            doc.set_head(gap, false);
            doc.type_text("");
            let mut right = vec![0];
            doc.set_head(0, false);
            while doc.selection().head < text.len() && right.len() <= text.len() {
                doc.motion(Motion::Right, false);
                right.push(doc.selection().head);
            }
            assert_eq!(right, breaks, "Right over {text:?}");
            let mut left = vec![text.len()];
            while doc.selection().head > 0 && left.len() <= text.len() {
                doc.motion(Motion::Left, false);
                left.push(doc.selection().head);
            }
            left.reverse();
            assert_eq!(left, breaks, "Left over {text:?}");
            for i in 0..=text.len() {
                let snapped = doc.snap(i);
                let expect = *breaks.iter().rev().find(|&&b| b <= i).unwrap();
                assert_eq!(snapped, expect, "snap({i}) in {text:?}");
            }
        }
    }
}

// ---- moves, find ----

#[test]
fn word_and_line_moves_hand_cases() {
    let line = "hello, world  foo";
    assert_eq!(cursor::word_right(line, 0), Some(5));
    assert_eq!(cursor::word_right(line, 5), Some(12));
    assert_eq!(cursor::word_right(line, 14), Some(17));
    assert_eq!(cursor::word_right(line, 17), None);
    assert_eq!(cursor::word_left(line, 17), Some(14));
    assert_eq!(cursor::word_left(line, 14), Some(7));
    assert_eq!(cursor::word_left(line, 7), Some(0));
    assert_eq!(cursor::word_left(line, 0), None);

    let mut doc = Document::load(b"ab cd\nef").unwrap();
    doc.motion(Motion::WordRight, false);
    doc.motion(Motion::WordRight, false);
    assert_eq!(doc.selection().head, 5);
    doc.motion(Motion::WordRight, false);
    assert_eq!(
        doc.selection().head,
        6,
        "past the line end to the next line"
    );
    doc.motion(Motion::LineEnd, true);
    assert_eq!(doc.selection(), Selection { anchor: 6, head: 8 });
    assert_eq!(doc.selected_text(), "ef");
    doc.motion(Motion::Left, false);
    assert_eq!(
        doc.selection(),
        Selection::caret(6),
        "Left collapses a selection to its start"
    );
    doc.motion(Motion::WordLeft, false);
    assert_eq!(doc.selection().head, 5, "back over the line end");
    doc.motion(Motion::DocEnd, false);
    assert_eq!(doc.selection().head, 8);
}

#[test]
fn find_hand_cases() {
    let doc = Document::load("one Two one\u{20ac}two".as_bytes()).unwrap();
    assert_eq!(doc.find("one", 0, true, false), Some(0..3));
    assert_eq!(doc.find("one", 1, true, false), Some(8..11));
    assert_eq!(doc.find("one", 9, true, false), Some(0..3), "wraps round");
    assert_eq!(doc.find("two", 0, true, false), Some(14..17));
    assert_eq!(
        doc.find("two", 0, true, true),
        Some(4..7),
        "ASCII case folded"
    );
    assert_eq!(doc.find("one", 11, false, false), Some(8..11));
    assert_eq!(doc.find("one", 10, false, false), Some(0..3));
    assert_eq!(
        doc.find("one", 2, false, false),
        Some(8..11),
        "wraps round backwards"
    );
    assert_eq!(doc.find("\u{20ac}", 0, true, false), Some(11..14));
    assert_eq!(doc.find("zzz", 0, true, false), None);
    assert_eq!(doc.find("", 0, true, false), None);
}

// ---- the text test and the file format ----

#[test]
fn text_test_hand_cases() {
    assert_eq!(format::check(b"hello\n"), Ok(()));
    assert_eq!(format::check(b""), Ok(()));
    assert_eq!(format::check("\u{e9}\u{1f600}".as_bytes()), Ok(()));
    assert_eq!(format::check(b"a\xffb"), Err(NotText::InvalidUtf8));
    assert_eq!(
        format::check(b"\xe2\x82"),
        Err(NotText::InvalidUtf8),
        "a cut-off char"
    );
    assert_eq!(
        format::check(b"\xed\xa0\x80"),
        Err(NotText::InvalidUtf8),
        "a UTF-16 surrogate"
    );
    assert_eq!(
        format::check(b"\xc0\x80"),
        Err(NotText::InvalidUtf8),
        "an overlong NUL"
    );
    assert_eq!(format::check(b"a\0b"), Err(NotText::Nul));
    let max = vec![b'a'; format::MAX_FILE_BYTES];
    assert_eq!(format::check(&max), Ok(()));
    let over = vec![b'a'; format::MAX_FILE_BYTES + 1];
    assert_eq!(format::check(&over), Err(NotText::TooLarge));
    assert!(Document::load(b"bin\0ary").is_err());
}

/// Hand-laid bytes: each file round-trips exactly, or (mixed) as stated.
#[test]
fn format_round_trips_byte_for_byte() {
    /// (file bytes, editor text, line ending, mixed, bytes saved back)
    type Case<'a> = (&'a [u8], &'a str, LineEnding, bool, &'a [u8]);
    let cases: &[Case] = &[
        (b"a\nb\n", "a\nb\n", LineEnding::Lf, false, b"a\nb\n"),
        (
            b"a\r\nb\r\n",
            "a\nb\n",
            LineEnding::Crlf,
            false,
            b"a\r\nb\r\n",
        ),
        (
            b"\xef\xbb\xbfa\r\nb",
            "a\nb",
            LineEnding::Crlf,
            false,
            b"\xef\xbb\xbfa\r\nb",
        ),
        (
            b"\xef\xbb\xbfa\nb",
            "a\nb",
            LineEnding::Lf,
            false,
            b"\xef\xbb\xbfa\nb",
        ),
        (
            b"lone\rcr\n",
            "lone\rcr\n",
            LineEnding::Lf,
            false,
            b"lone\rcr\n",
        ),
        (
            b"x\r\r\ny\r\n",
            "x\r\ny\n",
            LineEnding::Crlf,
            false,
            b"x\r\r\ny\r\n",
        ),
        (
            b"no line end",
            "no line end",
            LineEnding::Lf,
            false,
            b"no line end",
        ),
        (
            b"mixed\r\nends\n",
            "mixed\nends\n",
            LineEnding::Lf,
            true,
            b"mixed\nends\n",
        ),
    ];
    for &(file, text, ending, mixed, saved) in cases {
        let (got, fmt) = format::load(file).unwrap();
        assert_eq!(
            (got.as_str(), fmt.ending, fmt.mixed),
            (text, ending, mixed),
            "{file:?}"
        );
        assert_eq!(format::save(&got, &fmt), saved, "{file:?}");
        let doc = Document::load(file).unwrap();
        assert_eq!(doc.save_bytes(), saved);
    }
}

// ---- wrap ----

fn wrap(s: &str, width: f32) -> Vec<&str> {
    wrap::wrap_line(&FixedAdvance(1.0), s, width)
        .into_iter()
        .map(|r| &s[r])
        .collect::<Vec<_>>()
}

#[test]
fn wrap_hand_cases() {
    assert_eq!(wrap("hello world foo", 10.0), ["hello ", "world foo"]);
    assert_eq!(
        wrap("hello world foo", 11.0),
        ["hello world ", "foo"],
        "\"hello world\" is 11 wide"
    );
    assert_eq!(
        wrap("hello world", 5.0),
        ["hello ", "world"],
        "the space hangs"
    );
    assert_eq!(
        wrap("abcdefghij", 4.0),
        ["abcd", "efgh", "ij"],
        "a long word breaks by grapheme"
    );
    assert_eq!(wrap("", 4.0), [""]);
    assert_eq!(
        wrap("e\u{301}e\u{301}e\u{301}", 2.0),
        ["e\u{301}e\u{301}", "e\u{301}"],
        "a combining mark stays"
    );
    assert_eq!(
        wrap("a\tb", 4.0),
        ["a\t", "b"],
        "a tab is four columns and hangs"
    );
    assert_eq!(
        wrap("x", 0.0),
        ["x"],
        "one grapheme wider than the row still fits"
    );
}

#[test]
fn wrapped_rows_cover_the_line_and_fit() {
    let mut rng = Rng(77);
    for _ in 0..300 {
        let mut line = String::new();
        for _ in 0..rng.below(40) {
            line.push_str(rng.pick(&["ab", " ", "\u{e9}", "e\u{301}", "word", "\u{1f600}", "\t"]));
        }
        let width = 1.0 + rng.below(12) as f32;
        let rows = wrap::wrap_line(&FixedAdvance(1.0), &line, width);
        let joined: String = rows.iter().map(|r| &line[r.clone()]).collect();
        assert_eq!(joined, line);
        for r in &rows {
            assert!(!r.is_empty() || line.is_empty());
            let row = &line[r.clone()];
            let content = row.trim_end_matches([' ', '\t']);
            let mut stops = Vec::new();
            wrap::Measure::boundaries(&FixedAdvance(1.0), content, &mut stops);
            let w = stops.last().unwrap().1;
            assert!(
                w <= width || stops.len() == 2,
                "row {row:?} is {w} wide at {width}"
            );
        }
    }
}

#[test]
fn vertical_moves_keep_the_goal_column() {
    let measure = FixedAdvance(1.0);
    let doc = Document::load(b"hello world\nab\nlonger line").unwrap();
    let mut cache = WrapCache::new(6.0, doc.lines().count());
    // Rows: "hello " | "world" | "ab" | "longer " | "line".
    assert_eq!(
        cache.row_of(&doc, &measure, 6),
        (0, 1),
        "a row's end is the next row's start"
    );
    assert_eq!(
        cache.row_of(&doc, &measure, 11),
        (0, 1),
        "the line's end stays on its row"
    );
    let at = 3; // "hel|lo"
    let x = cache.x_of(&doc, &measure, at);
    assert_eq!(x, 3.0);
    let down = cache.vertical(&doc, &measure, at, 1, x);
    assert_eq!(down, 9, "\"wor|ld\"");
    let down = cache.vertical(&doc, &measure, down, 1, x);
    assert_eq!(
        down, 14,
        "\"ab|\": the nearest stop to x = 3 on a short row"
    );
    let down = cache.vertical(&doc, &measure, down, 1, x);
    assert_eq!(down, 18, "\"lon|ger\"");
    assert_eq!(
        cache.vertical(&doc, &measure, down, 5, x),
        doc.len(),
        "clamped at the end"
    );
    assert_eq!(cache.vertical(&doc, &measure, 9, -1, x), 3);
    assert_eq!(
        cache.vertical(&doc, &measure, 9, -9, x),
        0,
        "clamped at the start"
    );
    // A wrapped row's last stop belongs to the next row, so a goal past
    // the end of "hello " lands before its hanging space's end.
    assert_eq!(cache.vertical(&doc, &measure, 9, -1, 99.0), 5);
}

#[test]
fn wrap_cache_forgets_only_the_edited_lines() {
    let measure = FixedAdvance(1.0);
    let mut doc = Document::load(b"one\ntwo\nthree").unwrap();
    let mut cache = WrapCache::new(10.0, doc.lines().count());
    for line in 0..3 {
        cache.rows(&doc, &measure, line);
    }
    doc.set_head(5, false);
    let change = doc.type_text("X\nY");
    cache.edit(change);
    assert_eq!((change.first, change.old, change.new), (1, 1, 2));
    assert!(cache.is_cached(0) && !cache.is_cached(1) && !cache.is_cached(2) && cache.is_cached(3));
    assert_eq!(
        cache.rows(&doc, &measure, 2),
        core::slice::from_ref(&(0..3))
    );
    cache.set_width(3.0);
    assert!(!cache.is_cached(0));
}
