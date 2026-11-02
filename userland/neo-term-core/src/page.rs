// SPDX-License-Identifier: MPL-2.0
//! The term page (THINK L1a §3.2 and §3.4): a 4 KiB buffer the terminal and
//! its child share. Little-endian, fixed offsets, the rest zero; each word
//! has one writer.
//!
//! | Offset | Field | Writer |
//! |---|---|---|
//! | 0 | magic `"NTP1"` | terminal, once |
//! | 4 | version (u16), reserved (u16) | terminal, once |
//! | 8 | `size_seq` (seqlock, odd while writing) | terminal |
//! | 12 | columns (u16), rows (u16) | terminal |
//! | 16 | width px (u16), height px (u16) | terminal |
//! | 20 | `mode_req` | child |
//! | 24 | `mode_req_seq` | child |
//! | 28 | `mode_ack_seq` | terminal |
//! | 32 | `mode_cur` | terminal |
//!
//! Both sides touch the page only through aligned atomic loads and stores
//! ([`Page`]), never plain references, so neither side's view is a data race.

use core::sync::atomic::{AtomicU32, Ordering, fence};

pub const PAGE_LEN: usize = 4096;
pub const WORDS: usize = PAGE_LEN / 4;
/// Little-endian bytes `"NTP1"`.
pub const MAGIC: u32 = 0x3150_544E;
pub const VERSION: u16 = 1;

const W_MAGIC: usize = 0;
const W_VERSION: usize = 1;
const W_SIZE_SEQ: usize = 2;
const W_CELLS: usize = 3;
const W_PIXELS: usize = 4;
const W_MODE_REQ: usize = 5;
const W_MODE_REQ_SEQ: usize = 6;
const W_MODE_ACK_SEQ: usize = 7;
const W_MODE_CUR: usize = 8;

/// How many times a size read retries a torn or in-progress read before it
/// gives up (§3.2: then the caller keeps its last consistent read).
pub const SIZE_READ_TRIES: usize = 8;

/// The line discipline's mode (§4). Bit 0 cooked, bit 1 echo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mode {
    pub cooked: bool,
    pub echo: bool,
}

impl Mode {
    /// What every child starts in (§3.4).
    pub const COOKED_ECHO: Mode = Mode {
        cooked: true,
        echo: true,
    };
    pub const RAW: Mode = Mode {
        cooked: false,
        echo: false,
    };

    pub const fn bits(self) -> u32 {
        (self.cooked as u32) | ((self.echo as u32) << 1)
    }

    /// Masked to bits 0-1: the reserved bits of a child-written word are
    /// ignored (the terminal treats it as hostile).
    pub const fn from_bits(bits: u32) -> Mode {
        Mode {
            cooked: bits & 1 != 0,
            echo: bits & 2 != 0,
        }
    }
}

/// The window's size as the page carries it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size {
    pub cols: u16,
    pub rows: u16,
    pub width_px: u16,
    pub height_px: u16,
}

/// A snapshot of every field, for the byte oracle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Image {
    pub size_seq: u32,
    pub size: Size,
    pub mode_req: u32,
    pub mode_req_seq: u32,
    pub mode_ack_seq: u32,
    pub mode_cur: u32,
}

/// The page's bytes for `image` (magic and version filled in, the rest zero).
pub fn encode(image: &Image) -> [u8; PAGE_LEN] {
    let mut out = [0u8; PAGE_LEN];
    let words = words_of(image);
    for (i, w) in words.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&w.to_le_bytes());
    }
    out
}

/// The image in `bytes`, or `None` if the magic or version is wrong.
pub fn decode(bytes: &[u8; PAGE_LEN]) -> Option<Image> {
    let word = |i: usize| u32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap());
    if word(W_MAGIC) != MAGIC || word(W_VERSION) & 0xFFFF != VERSION as u32 {
        return None;
    }
    let (cols, rows) = split(word(W_CELLS));
    let (width_px, height_px) = split(word(W_PIXELS));
    Some(Image {
        size_seq: word(W_SIZE_SEQ),
        size: Size {
            cols,
            rows,
            width_px,
            height_px,
        },
        mode_req: word(W_MODE_REQ),
        mode_req_seq: word(W_MODE_REQ_SEQ),
        mode_ack_seq: word(W_MODE_ACK_SEQ),
        mode_cur: word(W_MODE_CUR),
    })
}

fn words_of(image: &Image) -> [u32; 9] {
    [
        MAGIC,
        VERSION as u32,
        image.size_seq,
        join(image.size.cols, image.size.rows),
        join(image.size.width_px, image.size.height_px),
        image.mode_req,
        image.mode_req_seq,
        image.mode_ack_seq,
        image.mode_cur,
    ]
}

const fn join(lo: u16, hi: u16) -> u32 {
    lo as u32 | (hi as u32) << 16
}

const fn split(w: u32) -> (u16, u16) {
    (w as u16, (w >> 16) as u16)
}

/// The steps of [`Page::write_size_with`], for a test that runs the reader
/// between them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SizeStep {
    /// `size_seq` is odd.
    Opened,
    /// The cells word is written.
    Cells,
    /// The pixels word is written.
    Pixels,
}

/// The page as both sides use it: aligned atomic words.
pub struct Page<'a> {
    words: &'a [AtomicU32],
}

impl<'a> Page<'a> {
    /// `words` must be the page's 1024 words.
    pub fn new(words: &'a [AtomicU32]) -> Option<Self> {
        (words.len() == WORDS).then_some(Self { words })
    }

    fn w(&self, i: usize) -> &AtomicU32 {
        &self.words[i]
    }

    // ── terminal side ─────────────────────────────────────────────────────

    /// The terminal sets up a fresh page for a new child: magic, version, the
    /// size, and the reset mode (§3.4: cooked with echo), with the request
    /// and ack sequences equal.
    pub fn init(&self, size: Size) {
        for w in self.words {
            w.store(0, Ordering::Relaxed);
        }
        self.w(W_MAGIC).store(MAGIC, Ordering::Relaxed);
        self.w(W_VERSION).store(VERSION as u32, Ordering::Relaxed);
        self.w(W_MODE_CUR)
            .store(Mode::COOKED_ECHO.bits(), Ordering::Relaxed);
        self.write_size(size);
    }

    /// The terminal writes the size (§3.2's seqlock rows).
    pub fn write_size(&self, size: Size) {
        self.write_size_with(size, |_| {});
    }

    /// [`Page::write_size`], calling `between` after each step.
    pub fn write_size_with(&self, size: Size, mut between: impl FnMut(SizeStep)) {
        let seq = self.w(W_SIZE_SEQ).load(Ordering::Relaxed);
        // Odd: a write is in progress.
        self.w(W_SIZE_SEQ)
            .store(seq.wrapping_add(1) | 1, Ordering::Relaxed);
        fence(Ordering::Release);
        between(SizeStep::Opened);
        self.w(W_CELLS)
            .store(join(size.cols, size.rows), Ordering::Relaxed);
        between(SizeStep::Cells);
        self.w(W_PIXELS)
            .store(join(size.width_px, size.height_px), Ordering::Relaxed);
        between(SizeStep::Pixels);
        // Even again, and one past the odd value.
        let odd = self.w(W_SIZE_SEQ).load(Ordering::Relaxed);
        self.w(W_SIZE_SEQ)
            .store(odd.wrapping_add(1), Ordering::Release);
    }

    /// The terminal checks for a mode request (§3.2: on every wake). Returns
    /// the requested mode and its sequence if it changed since `last`.
    pub fn poll_mode_request(&self, last: u32) -> Option<(Mode, u32)> {
        let q = self.w(W_MODE_REQ_SEQ).load(Ordering::Acquire);
        if q == last {
            return None;
        }
        let bits = self.w(W_MODE_REQ).load(Ordering::Relaxed);
        Some((Mode::from_bits(bits), q))
    }

    /// The terminal publishes the mode now in force and acknowledges
    /// request `q` (`mode_cur` before the ack's Release store).
    pub fn ack_mode(&self, mode: Mode, q: u32) {
        self.w(W_MODE_CUR).store(mode.bits(), Ordering::Relaxed);
        self.w(W_MODE_ACK_SEQ).store(q, Ordering::Release);
    }

    // ── child side ────────────────────────────────────────────────────────

    /// The page's magic and version are right (`is_terminal`'s check).
    pub fn is_valid(&self) -> bool {
        self.w(W_MAGIC).load(Ordering::Acquire) == MAGIC
            && self.w(W_VERSION).load(Ordering::Relaxed) & 0xFFFF == VERSION as u32
    }

    /// One consistent read of the size and its (even) sequence, or `None`
    /// if [`SIZE_READ_TRIES`] tries all met a write in progress.
    pub fn read_size(&self) -> Option<(Size, u32)> {
        for _ in 0..SIZE_READ_TRIES {
            let s1 = self.w(W_SIZE_SEQ).load(Ordering::Acquire);
            if s1 & 1 != 0 {
                core::hint::spin_loop();
                continue;
            }
            let (cols, rows) = split(self.w(W_CELLS).load(Ordering::Relaxed));
            let (width_px, height_px) = split(self.w(W_PIXELS).load(Ordering::Relaxed));
            fence(Ordering::Acquire);
            let s2 = self.w(W_SIZE_SEQ).load(Ordering::Relaxed);
            if s1 == s2 {
                return Some((
                    Size {
                        cols,
                        rows,
                        width_px,
                        height_px,
                    },
                    s1,
                ));
            }
        }
        None
    }

    /// The child asks for `mode`; returns the request's sequence `q`. The
    /// caller then waits until [`Page::acked`] reaches `q` (a futex wait on
    /// [`Page::ack_word`], with its 1 s deadline).
    pub fn request_mode(&self, mode: Mode) -> u32 {
        self.w(W_MODE_REQ).store(mode.bits(), Ordering::Relaxed);
        let q = self
            .w(W_MODE_REQ_SEQ)
            .load(Ordering::Relaxed)
            .wrapping_add(1);
        self.w(W_MODE_REQ_SEQ).store(q, Ordering::Release);
        q
    }

    /// Whether request `q` is acknowledged (wrapping comparison).
    pub fn acked(&self, q: u32) -> bool {
        let a = self.w(W_MODE_ACK_SEQ).load(Ordering::Acquire);
        a.wrapping_sub(q) as i32 >= 0
    }

    /// The word a child futex-waits on for the ack.
    pub fn ack_word(&self) -> &AtomicU32 {
        self.w(W_MODE_ACK_SEQ)
    }

    /// The mode in force (§3.4's `mode()`): Acquire on the ack, then
    /// `mode_cur`.
    pub fn mode(&self) -> Mode {
        let _ = self.w(W_MODE_ACK_SEQ).load(Ordering::Acquire);
        Mode::from_bits(self.w(W_MODE_CUR).load(Ordering::Relaxed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page_words() -> Vec<AtomicU32> {
        (0..WORDS).map(|_| AtomicU32::new(0)).collect()
    }

    const SIZE_80X25: Size = Size {
        cols: 80,
        rows: 25,
        width_px: 640,
        height_px: 400,
    };

    /// THINK §3.2's hand-laid oracle (revision 3): 80 × 25, 640 × 400 px,
    /// cooked with echo requested and in force, request 1 acknowledged,
    /// `size_seq` 2.
    const ORACLE_HEAD: [u8; 36] = [
        0x4e, 0x54, 0x50, 0x31, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x50, 0x00, 0x19,
        0x00, //
        0x80, 0x02, 0x90, 0x01, 0x03, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
        0x00, //
        0x03, 0x00, 0x00, 0x00,
    ];

    fn oracle_image() -> Image {
        Image {
            size_seq: 2,
            size: SIZE_80X25,
            mode_req: 3,
            mode_req_seq: 1,
            mode_ack_seq: 1,
            mode_cur: 3,
        }
    }

    #[test]
    fn encode_matches_the_hand_laid_bytes() {
        let bytes = encode(&oracle_image());
        assert_eq!(&bytes[..36], &ORACLE_HEAD[..]);
        assert!(bytes[36..].iter().all(|&b| b == 0));
    }

    #[test]
    fn decode_reads_the_hand_laid_bytes() {
        let mut bytes = [0u8; PAGE_LEN];
        bytes[..36].copy_from_slice(&ORACLE_HEAD);
        assert_eq!(decode(&bytes), Some(oracle_image()));
        bytes[0] = 0;
        assert_eq!(decode(&bytes), None);
    }

    /// The atomic protocol produces the oracle's bytes: init, then the child
    /// requests cooked+echo and the terminal acknowledges it.
    #[test]
    fn protocol_steps_produce_the_oracle_page() {
        let words = page_words();
        let page = Page::new(&words).unwrap();
        page.init(SIZE_80X25);
        let q = page.request_mode(Mode::COOKED_ECHO);
        let (mode, seen) = page.poll_mode_request(0).unwrap();
        page.ack_mode(mode, seen);
        assert!(page.acked(q));
        let mut bytes = [0u8; PAGE_LEN];
        for (i, w) in words.iter().enumerate() {
            bytes[i * 4..i * 4 + 4].copy_from_slice(&w.load(Ordering::Relaxed).to_le_bytes());
        }
        assert_eq!(&bytes[..36], &ORACLE_HEAD[..]);
        assert!(bytes[36..].iter().all(|&b| b == 0));
    }

    /// A reader run at every step of a write sees either the old size or
    /// nothing (a write in progress), never a mix.
    #[test]
    fn a_reader_between_any_two_write_steps_never_sees_a_torn_size() {
        let words = page_words();
        let page = Page::new(&words).unwrap();
        page.init(SIZE_80X25);
        let (before, seq0) = page.read_size().unwrap();
        assert_eq!(before, SIZE_80X25);
        assert_eq!(seq0 & 1, 0);
        let new = Size {
            cols: 132,
            rows: 43,
            width_px: 1056,
            height_px: 688,
        };
        let mut seen = Vec::new();
        page.write_size_with(new, |step| seen.push((step, page.read_size())));
        assert_eq!(seen.len(), 3);
        for (step, read) in seen {
            assert_eq!(read, None, "step {step:?} must not yield a size");
        }
        let (after, seq1) = page.read_size().unwrap();
        assert_eq!(after, new);
        assert_eq!(seq1, seq0 + 2);
    }

    #[test]
    fn a_new_page_starts_cooked_with_echo() {
        let words = page_words();
        let page = Page::new(&words).unwrap();
        page.init(SIZE_80X25);
        assert!(page.is_valid());
        assert_eq!(page.mode(), Mode::COOKED_ECHO);
        assert_eq!(page.poll_mode_request(0), None);
    }

    #[test]
    fn a_mode_change_is_seen_once_and_acked() {
        let words = page_words();
        let page = Page::new(&words).unwrap();
        page.init(SIZE_80X25);
        let q = page.request_mode(Mode::RAW);
        assert!(!page.acked(q));
        let (mode, seen) = page.poll_mode_request(0).unwrap();
        assert_eq!((mode, seen), (Mode::RAW, q));
        page.ack_mode(mode, seen);
        assert!(page.acked(q));
        assert_eq!(page.mode(), Mode::RAW);
        assert_eq!(page.poll_mode_request(seen), None);
    }

    /// The child-written word is hostile: reserved bits are ignored.
    #[test]
    fn reserved_mode_bits_are_masked() {
        assert_eq!(Mode::from_bits(0xFFFF_FFFC), Mode::RAW);
        assert_eq!(Mode::from_bits(0xFFFF_FFFF), Mode::COOKED_ECHO);
        let words = page_words();
        let page = Page::new(&words).unwrap();
        page.init(SIZE_80X25);
        words[W_MODE_REQ].store(0xDEAD_BEE1, Ordering::Relaxed);
        words[W_MODE_REQ_SEQ].store(1, Ordering::Release);
        let (mode, _) = page.poll_mode_request(0).unwrap();
        assert_eq!(
            mode,
            Mode {
                cooked: true,
                echo: false
            }
        );
    }

    #[test]
    fn a_page_of_the_wrong_length_is_refused() {
        let words: Vec<AtomicU32> = (0..10).map(|_| AtomicU32::new(0)).collect();
        assert!(Page::new(&words).is_none());
    }
}
