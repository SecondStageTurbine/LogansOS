// SPDX-License-Identifier: MPL-2.0
//! `Transfer-Encoding: chunked` (RFC 9112 §7.1), decoded as it arrives.
//!
//! Bytes come off the TLS stream in whatever pieces the records make, so the
//! decoder is a state machine fed one slice at a time: [`Chunked::feed`]
//! appends the body bytes it finds to `out` and says whether the final chunk
//! (and the trailer section after it) has been read. Chunk extensions and
//! trailer fields are skipped. Refused: a size line that isn't hex, a size
//! with more than [`MAX_SIZE_DIGITS`] digits, a missing CRLF after a chunk, a
//! body past the caller's limit, and an over-long size or trailer line.

use alloc::vec::Vec;

/// A chunk size of up to 2^32 - 1 bytes; longer size lines are refused.
pub const MAX_SIZE_DIGITS: usize = 8;
/// Longest size line or trailer line accepted (with extensions).
pub const MAX_LINE: usize = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChunkError {
    /// A size line with no hex digit, or a stray character in it.
    BadSize,
    /// More than [`MAX_SIZE_DIGITS`] hex digits.
    SizeTooLong,
    /// The chunk data was not followed by CRLF.
    MissingCrlf,
    /// A size or trailer line past [`MAX_LINE`].
    LineTooLong,
    /// The decoded body would pass the caller's limit.
    TooLarge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    /// Reading hex digits of a chunk size.
    Size,
    /// Past the digits: skipping an extension up to CR.
    Extension,
    /// Read CR after the size line; expect LF.
    SizeLf,
    /// Copying chunk data; this many bytes left.
    Data(u64),
    /// After the data: expect CR.
    DataCr,
    /// After the data's CR: expect LF.
    DataLf,
    /// After the last chunk: at the start of a trailer line (or the end).
    TrailerStart,
    /// In a trailer line, up to CR.
    Trailer,
    /// Read CR at the end of a trailer line; expect LF.
    TrailerLf,
    /// Read CR on an empty line: the last LF ends the message.
    EndLf,
    Done,
}

#[derive(Clone, Debug)]
pub struct Chunked {
    state: State,
    size: u64,
    digits: usize,
    line: usize,
    limit: usize,
    total: usize,
}

impl Chunked {
    /// A decoder that refuses a body longer than `limit` bytes.
    pub fn new(limit: usize) -> Self {
        Chunked {
            state: State::Size,
            size: 0,
            digits: 0,
            line: 0,
            limit,
            total: 0,
        }
    }

    pub fn is_done(&self) -> bool {
        self.state == State::Done
    }

    /// Feed the next bytes of the stream. Returns how many bytes of `input`
    /// were consumed (all of them unless the message ended part-way) and
    /// whether it has ended.
    pub fn feed(&mut self, input: &[u8], out: &mut Vec<u8>) -> Result<(usize, bool), ChunkError> {
        let mut i = 0;
        while i < input.len() && self.state != State::Done {
            let b = input[i];
            match self.state {
                State::Size => {
                    if let Some(d) = hex(b) {
                        self.digits += 1;
                        if self.digits > MAX_SIZE_DIGITS {
                            return Err(ChunkError::SizeTooLong);
                        }
                        self.size = self.size << 4 | u64::from(d);
                    } else if self.digits == 0 {
                        return Err(ChunkError::BadSize);
                    } else if b == b'\r' {
                        self.state = State::SizeLf;
                    } else if b == b';' || b == b' ' || b == b'\t' {
                        self.state = State::Extension;
                    } else {
                        return Err(ChunkError::BadSize);
                    }
                    self.bump_line()?;
                    i += 1;
                }
                State::Extension => {
                    if b == b'\r' {
                        self.state = State::SizeLf;
                    }
                    self.bump_line()?;
                    i += 1;
                }
                State::SizeLf => {
                    if b != b'\n' {
                        return Err(ChunkError::BadSize);
                    }
                    self.line = 0;
                    self.state = if self.size == 0 {
                        State::TrailerStart
                    } else {
                        if self.total as u64 + self.size > self.limit as u64 {
                            return Err(ChunkError::TooLarge);
                        }
                        State::Data(self.size)
                    };
                    i += 1;
                }
                State::Data(left) => {
                    let take = (input.len() - i).min(left as usize);
                    out.extend_from_slice(&input[i..i + take]);
                    self.total += take;
                    i += take;
                    let left = left - take as u64;
                    self.state = if left == 0 {
                        State::DataCr
                    } else {
                        State::Data(left)
                    };
                }
                State::DataCr => {
                    if b != b'\r' {
                        return Err(ChunkError::MissingCrlf);
                    }
                    self.state = State::DataLf;
                    i += 1;
                }
                State::DataLf => {
                    if b != b'\n' {
                        return Err(ChunkError::MissingCrlf);
                    }
                    self.size = 0;
                    self.digits = 0;
                    self.line = 0;
                    self.state = State::Size;
                    i += 1;
                }
                State::TrailerStart => {
                    self.state = if b == b'\r' {
                        State::EndLf
                    } else {
                        self.bump_line()?;
                        State::Trailer
                    };
                    i += 1;
                }
                State::Trailer => {
                    if b == b'\r' {
                        self.state = State::TrailerLf;
                    }
                    self.bump_line()?;
                    i += 1;
                }
                State::TrailerLf => {
                    if b != b'\n' {
                        return Err(ChunkError::MissingCrlf);
                    }
                    self.line = 0;
                    self.state = State::TrailerStart;
                    i += 1;
                }
                State::EndLf => {
                    if b != b'\n' {
                        return Err(ChunkError::MissingCrlf);
                    }
                    self.state = State::Done;
                    i += 1;
                }
                State::Done => unreachable!("loop condition"),
            }
        }
        Ok((i, self.is_done()))
    }

    fn bump_line(&mut self) -> Result<(), ChunkError> {
        self.line += 1;
        if self.line > MAX_LINE {
            return Err(ChunkError::LineTooLong);
        }
        Ok(())
    }
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    /// Byte oracle: RFC 9112's example shape, written out by hand. Two chunks
    /// (4 and 0x10 bytes), an extension, the last chunk and one trailer field.
    /// "pedia in chunks." is 5 + 3 + 8 = 16 = 0x10 bytes.
    const STREAM: &[u8] =
        b"4\r\nWiki\r\n10;name=value\r\npedia in chunks.\r\n0\r\nExpires: never\r\n\r\n";
    const BODY: &[u8] = b"Wikipedia in chunks.";

    #[test]
    fn whole_stream() {
        let mut d = Chunked::new(1 << 20);
        let mut out = Vec::new();
        assert_eq!(d.feed(STREAM, &mut out), Ok((STREAM.len(), true)));
        assert_eq!(out, BODY);
    }

    #[test]
    fn every_split_point_and_byte_at_a_time() {
        for cut in 0..=STREAM.len() {
            let mut d = Chunked::new(1 << 20);
            let mut out = Vec::new();
            let (a, done_a) = d.feed(&STREAM[..cut], &mut out).unwrap();
            assert_eq!(a, cut);
            assert!(!done_a || cut == STREAM.len());
            let (b, done) = d.feed(&STREAM[cut..], &mut out).unwrap();
            assert_eq!(a + b, STREAM.len(), "cut {cut}");
            assert!(done, "cut {cut}");
            assert_eq!(out, BODY, "cut {cut}");
        }
        let mut d = Chunked::new(1 << 20);
        let mut out = Vec::new();
        for byte in STREAM {
            d.feed(core::slice::from_ref(byte), &mut out).unwrap();
        }
        assert!(d.is_done());
        assert_eq!(out, BODY);
    }

    #[test]
    fn stops_at_the_end_of_the_message() {
        let mut stream = STREAM.to_vec();
        stream.extend_from_slice(b"HTTP/1.1 200 OK"); // the next response
        let mut d = Chunked::new(1 << 20);
        let mut out = Vec::new();
        assert_eq!(d.feed(&stream, &mut out), Ok((STREAM.len(), true)));
    }

    #[test]
    fn upper_case_hex_and_empty_body() {
        let mut d = Chunked::new(64);
        let mut out = Vec::new();
        assert_eq!(
            d.feed(b"A\r\n0123456789\r\n0\r\n\r\n", &mut out),
            Ok((20, true))
        );
        assert_eq!(out, b"0123456789");
        let mut d = Chunked::new(64);
        let mut out = Vec::new();
        assert_eq!(d.feed(b"0\r\n\r\n", &mut out), Ok((5, true)));
        assert!(out.is_empty());
    }

    #[test]
    fn refusals() {
        let cases: &[(&[u8], ChunkError)] = &[
            (b"\r\n", ChunkError::BadSize),
            (b"g\r\n", ChunkError::BadSize),
            (b"4x\r\n", ChunkError::BadSize),
            (b"4\rX", ChunkError::BadSize),
            (b"123456789\r\n", ChunkError::SizeTooLong),
            (b"4\r\nWikiX", ChunkError::MissingCrlf),
            (b"4\r\nWiki\rX", ChunkError::MissingCrlf),
            (b"0\r\n\rX", ChunkError::MissingCrlf),
        ];
        for (input, want) in cases {
            let mut d = Chunked::new(1 << 20);
            let mut out = Vec::new();
            assert_eq!(d.feed(input, &mut out), Err(*want), "{input:?}");
        }
    }

    #[test]
    fn limit_is_checked_before_the_data_arrives() {
        let mut d = Chunked::new(8);
        let mut out = Vec::new();
        assert_eq!(d.feed(b"4\r\nWiki\r\n", &mut out), Ok((9, false)));
        // 4 + 5 > 8: refused at the size line, nothing more copied.
        assert_eq!(d.feed(b"5\r\n", &mut out), Err(ChunkError::TooLarge));
        assert_eq!(out, b"Wiki");
        // A size near u32::MAX never reaches an allocation.
        let mut d = Chunked::new(1 << 20);
        assert_eq!(
            d.feed(b"ffffffff\r\n", &mut vec![]),
            Err(ChunkError::TooLarge)
        );
    }

    #[test]
    fn long_lines_are_refused() {
        let mut line = vec![b'1', b';'];
        line.resize(MAX_LINE + 8, b'x');
        let mut d = Chunked::new(1 << 20);
        assert_eq!(d.feed(&line, &mut vec![]), Err(ChunkError::LineTooLong));
    }
}
