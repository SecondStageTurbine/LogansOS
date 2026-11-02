// SPDX-License-Identifier: MPL-2.0
//! An HTTP/1.1 response head (RFC 9112 §4–§6), and how its body is framed.
//!
//! [`parse_head`] reads the status line and header fields once the blank line
//! that ends them has arrived, and decides the body framing (RFC 9112 §6.3).
//! The broker sits between untrusted servers and content, so the ambiguous
//! cases that response splitting and desync attacks rely on are refused, not
//! resolved:
//! - two `Content-Length` values that differ, or one that isn't a number;
//! - `Content-Length` together with `Transfer-Encoding`;
//! - a `Transfer-Encoding` whose last coding is not `chunked`;
//! - an obs-fold continuation line, a bare CR or LF, a field name that isn't a
//!   token, or a head past [`MAX_HEAD`].

use alloc::string::String;
use alloc::vec::Vec;

use crate::wire::Header;

/// Longest response head accepted (status line plus fields).
pub const MAX_HEAD: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Framing {
    /// No body: a `HEAD` request, or status 1xx, 204 or 304.
    None,
    /// Exactly this many bytes.
    Length(u64),
    /// `Transfer-Encoding: chunked` ([`crate::chunked`]).
    Chunked,
    /// The body runs until the server closes the connection.
    Close,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Head {
    pub status: u16,
    pub headers: Vec<Header>,
    pub framing: Framing,
}

impl Head {
    /// The first value of a field, by case-insensitive name.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case(name))
            .map(|h| h.value.as_str())
    }

    /// Whether the status is a redirect the broker follows.
    pub fn is_redirect(&self) -> bool {
        matches!(self.status, 301 | 302 | 303 | 307 | 308)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HttpError {
    /// Not `HTTP/1.x NNN ...`.
    StatusLine,
    /// A field line that isn't `token ":" value`, or obs-fold.
    Field,
    /// A CR not followed by LF, or an LF not preceded by CR.
    LineEnding,
    /// Conflicting or malformed `Content-Length`.
    ContentLength,
    /// `Transfer-Encoding` the broker can't frame, or with `Content-Length`.
    TransferEncoding,
    /// The head passed [`MAX_HEAD`], or more fields than the wire carries.
    TooLarge,
}

/// Parse a response head from the start of `buf`. `Ok(None)`: the head isn't
/// complete yet. `Ok(Some((head, n)))`: the body starts at `buf[n]`.
/// `head_request` says the request was `HEAD` (so no body follows).
pub fn parse_head(buf: &[u8], head_request: bool) -> Result<Option<(Head, usize)>, HttpError> {
    let Some(end) = find(buf, b"\r\n\r\n") else {
        if buf.len() > MAX_HEAD {
            return Err(HttpError::TooLarge);
        }
        return Ok(None);
    };
    if end + 4 > MAX_HEAD {
        return Err(HttpError::TooLarge);
    }
    let head = &buf[..end];
    // Every LF must close a CRLF and every CR open one.
    for (i, &b) in head.iter().enumerate() {
        let bad_cr = b == b'\r' && head.get(i + 1) != Some(&b'\n');
        let bad_lf = b == b'\n' && (i == 0 || head[i - 1] != b'\r');
        if bad_cr || bad_lf {
            return Err(HttpError::LineEnding);
        }
    }
    let mut lines = head
        .split(|&b| b == b'\n')
        .map(|l| l.strip_suffix(b"\r").unwrap_or(l));
    let status = status_line(lines.next().unwrap_or(b""))?;
    let mut headers = Vec::new();
    for line in lines {
        if line.first().is_some_and(|&b| b == b' ' || b == b'\t') {
            return Err(HttpError::Field); // obs-fold
        }
        let colon = line
            .iter()
            .position(|&b| b == b':')
            .ok_or(HttpError::Field)?;
        let name = &line[..colon];
        if name.is_empty() || !name.iter().all(|&b| is_tchar(b)) {
            return Err(HttpError::Field);
        }
        let value = trim_ows(&line[colon + 1..]);
        if value.iter().any(|&b| b == 0 || b == b'\r' || b == b'\n') {
            return Err(HttpError::Field);
        }
        if headers.len() == crate::wire::MAX_HEADERS
            || name.len() > crate::wire::MAX_HEADER_NAME
            || value.len() > crate::wire::MAX_HEADER_VALUE
        {
            return Err(HttpError::TooLarge);
        }
        headers.push(Header {
            name: latin1(name),
            value: latin1(value),
        });
    }
    let framing = framing(status, &headers, head_request)?;
    Ok(Some((
        Head {
            status,
            headers,
            framing,
        },
        end + 4,
    )))
}

fn status_line(line: &[u8]) -> Result<u16, HttpError> {
    let rest = line
        .strip_prefix(b"HTTP/1.1 ")
        .or_else(|| line.strip_prefix(b"HTTP/1.0 "))
        .ok_or(HttpError::StatusLine)?;
    let code = rest.get(..3).ok_or(HttpError::StatusLine)?;
    if !code.iter().all(u8::is_ascii_digit) || code[0] == b'0' {
        return Err(HttpError::StatusLine);
    }
    if rest.len() > 3 && rest[3] != b' ' {
        return Err(HttpError::StatusLine);
    }
    Ok(
        u16::from(code[0] - b'0') * 100
            + u16::from(code[1] - b'0') * 10
            + u16::from(code[2] - b'0'),
    )
}

fn framing(status: u16, headers: &[Header], head_request: bool) -> Result<Framing, HttpError> {
    let mut length: Option<u64> = None;
    let mut transfer: Option<&str> = None;
    for h in headers {
        if h.name.eq_ignore_ascii_case("content-length") {
            // A list of identical values is the same length (RFC 9110 §8.6).
            for part in h.value.split(',') {
                let part = part.trim_matches([' ', '\t']);
                if part.is_empty() || part.len() > 19 || !part.bytes().all(|b| b.is_ascii_digit()) {
                    return Err(HttpError::ContentLength);
                }
                let n: u64 = part.parse().map_err(|_| HttpError::ContentLength)?;
                match length {
                    Some(prev) if prev != n => return Err(HttpError::ContentLength),
                    _ => length = Some(n),
                }
            }
        } else if h.name.eq_ignore_ascii_case("transfer-encoding") {
            if transfer.is_some() {
                return Err(HttpError::TransferEncoding);
            }
            transfer = Some(&h.value);
        }
    }
    if head_request || (100..200).contains(&status) || status == 204 || status == 304 {
        return Ok(Framing::None);
    }
    match (transfer, length) {
        (Some(_), Some(_)) => Err(HttpError::TransferEncoding),
        (Some(te), None) => {
            let last = te
                .rsplit(',')
                .next()
                .unwrap_or("")
                .trim_matches([' ', '\t']);
            if last.eq_ignore_ascii_case("chunked") {
                Ok(Framing::Chunked)
            } else {
                Err(HttpError::TransferEncoding)
            }
        }
        (None, Some(n)) => Ok(Framing::Length(n)),
        (None, None) => Ok(Framing::Close),
    }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

fn is_tchar(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b)
}

fn trim_ows(mut v: &[u8]) -> &[u8] {
    while let [b' ' | b'\t', rest @ ..] = v {
        v = rest;
    }
    while let [rest @ .., b' ' | b'\t'] = v {
        v = rest;
    }
    v
}

/// Field values are bytes; anything outside ASCII maps through Latin-1, as
/// the Fetch standard's "isomorphic decode" does.
fn latin1(b: &[u8]) -> String {
    b.iter().map(|&c| char::from(c)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn head(raw: &[u8]) -> Result<Option<(Head, usize)>, HttpError> {
        parse_head(raw, false)
    }

    #[test]
    fn a_complete_head_and_where_the_body_starts() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 5\r\n\r\nhello";
        let (h, n) = head(raw).unwrap().unwrap();
        assert_eq!(h.status, 200);
        assert_eq!(h.framing, Framing::Length(5));
        assert_eq!(h.get("content-type"), Some("text/html"));
        assert_eq!(&raw[n..], b"hello");
        assert_eq!(n, raw.len() - 5);
    }

    #[test]
    fn incomplete_heads_wait() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\n";
        for cut in 0..raw.len() {
            assert_eq!(head(&raw[..cut]), Ok(None), "cut {cut}");
        }
        assert!(head(raw).unwrap().is_some());
    }

    #[test]
    fn framing_rules() {
        let f = |raw: &[u8], head_req| parse_head(raw, head_req).unwrap().unwrap().0.framing;
        assert_eq!(
            f(
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n",
                false
            ),
            Framing::Chunked
        );
        assert_eq!(
            f(
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: gzip, chunked\r\n\r\n",
                false
            ),
            Framing::Chunked
        );
        assert_eq!(f(b"HTTP/1.1 200 OK\r\n\r\n", false), Framing::Close);
        assert_eq!(f(b"HTTP/1.0 200 OK\r\n\r\n", false), Framing::Close);
        assert_eq!(
            f(
                b"HTTP/1.1 204 No Content\r\nContent-Length: 9\r\n\r\n",
                false
            ),
            Framing::None
        );
        assert_eq!(
            f(b"HTTP/1.1 304 Not Modified\r\n\r\n", false),
            Framing::None
        );
        assert_eq!(
            f(b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\n", true),
            Framing::None
        );
        assert_eq!(
            f(b"HTTP/1.1 200 OK\r\nContent-Length: 7, 7\r\n\r\n", false),
            Framing::Length(7)
        );
        assert_eq!(
            f(
                b"HTTP/1.1 200 OK\r\ncontent-length: 7\r\nContent-Length: 7\r\n\r\n",
                false
            ),
            Framing::Length(7)
        );
    }

    #[test]
    fn redirects() {
        let (h, _) = head(b"HTTP/1.1 301 Moved\r\nLocation: /x\r\nContent-Length: 0\r\n\r\n")
            .unwrap()
            .unwrap();
        assert!(h.is_redirect());
        assert_eq!(h.get("LOCATION"), Some("/x"));
        let (h, _) = head(b"HTTP/1.1 300 Multiple\r\n\r\n").unwrap().unwrap();
        assert!(!h.is_redirect());
    }

    #[test]
    fn smuggling_shapes_are_refused() {
        let cases: &[(&[u8], HttpError)] = &[
            (b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nContent-Length: 6\r\n\r\n", HttpError::ContentLength),
            (b"HTTP/1.1 200 OK\r\nContent-Length: 5, 6\r\n\r\n", HttpError::ContentLength),
            (b"HTTP/1.1 200 OK\r\nContent-Length: +5\r\n\r\n", HttpError::ContentLength),
            (b"HTTP/1.1 200 OK\r\nContent-Length: 0x5\r\n\r\n", HttpError::ContentLength),
            (b"HTTP/1.1 200 OK\r\nContent-Length:\r\n\r\n", HttpError::ContentLength),
            (
                b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nTransfer-Encoding: chunked\r\n\r\n",
                HttpError::TransferEncoding,
            ),
            (b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked, gzip\r\n\r\n", HttpError::TransferEncoding),
            (b"HTTP/1.1 200 OK\r\nTransfer-Encoding: identity\r\n\r\n", HttpError::TransferEncoding),
            (
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nTransfer-Encoding: chunked\r\n\r\n",
                HttpError::TransferEncoding,
            ),
            (b"HTTP/1.1 200 OK\r\nX: a\r\n folded\r\n\r\n", HttpError::Field),
            (b"HTTP/1.1 200 OK\r\nBad Name: a\r\n\r\n", HttpError::Field),
            (b"HTTP/1.1 200 OK\r\nNoColon\r\n\r\n", HttpError::Field),
            (b"HTTP/1.1 200 OK\nX: a\r\n\r\n", HttpError::LineEnding),
            (b"HTTP/1.1 200 OK\r\nX: a\rb\r\n\r\n", HttpError::LineEnding),
            (b"HTTP/2 200\r\n\r\n", HttpError::StatusLine),
            (b"HTTP/1.1 20 OK\r\n\r\n", HttpError::StatusLine),
            (b"HTTP/1.1 2000 OK\r\n\r\n", HttpError::StatusLine),
            (b"HTTP/1.1 099 Low\r\n\r\n", HttpError::StatusLine),
        ];
        for (raw, want) in cases {
            assert_eq!(head(raw), Err(*want), "{:?}", core::str::from_utf8(raw));
        }
    }

    #[test]
    fn limits() {
        let mut big = b"HTTP/1.1 200 OK\r\nX: ".to_vec();
        big.resize(MAX_HEAD + 1, b'a');
        assert_eq!(head(&big), Err(HttpError::TooLarge));
        let mut many = b"HTTP/1.1 200 OK\r\n".to_vec();
        for i in 0..=crate::wire::MAX_HEADERS {
            many.extend_from_slice(alloc::format!("X{i}: v\r\n").as_bytes());
        }
        many.extend_from_slice(b"\r\n");
        assert_eq!(head(&many), Err(HttpError::TooLarge));
    }

    #[test]
    fn values_are_trimmed_and_isomorphic() {
        let (h, _) = head(b"HTTP/1.1 200 OK\r\nX:  \tcaf\xe9 \t\r\nY:\r\n\r\n")
            .unwrap()
            .unwrap();
        assert_eq!(h.get("x"), Some("caf\u{e9}"));
        assert_eq!(h.get("y"), Some(""));
    }
}
