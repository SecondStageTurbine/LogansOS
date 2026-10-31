// SPDX-License-Identifier: MPL-2.0
//! The fetch messages (THINK §2, "The wire format").
//!
//! One message per channel `send`, little-endian:
//!
//! | Message | Layout |
//! |---|---|
//! | `FETCH` (content → broker) | `u8 1`, `u32 id`, `u8 method`, `u16 url_len`, url, headers |
//! | `HEAD` (broker → content) | `u8 2`, `u32 id`, `u16 status`, `u32 content_length` (`u32::MAX` unknown), headers, `u16 final_url_len`, final url |
//! | `BODY` (broker → content) | `u8 3`, `u32 id`, `u16 len`, bytes |
//! | `END` (broker → content) | `u8 4`, `u32 id` |
//! | `ERROR` (broker → content) | `u8 5`, `u32 id`, `u8 code` |
//! | `CANCEL` (content → broker) | `u8 6`, `u32 id` |
//!
//! `headers` is `u16 count`, then per header `u16 name_len`, name, `u16
//! value_len`, value. Decoding refuses anything past the limits below, a
//! truncated or over-long message, an unknown tag, method or error code, and
//! non-UTF-8 text.

use alloc::string::String;
use alloc::vec::Vec;

/// Longest URL a `FETCH` or `HEAD` carries (bytes).
pub const MAX_URL: usize = 2048;
/// Most headers in one message.
pub const MAX_HEADERS: usize = 64;
/// Longest header name (bytes).
pub const MAX_HEADER_NAME: usize = 256;
/// Longest header value (bytes).
pub const MAX_HEADER_VALUE: usize = 4096;
/// Largest `BODY` payload (bytes): one message fits a channel send.
pub const MAX_BODY_CHUNK: usize = 4096;
/// `HEAD.content_length` when the length is not known.
pub const LENGTH_UNKNOWN: u32 = u32::MAX;

const TAG_FETCH: u8 = 1;
const TAG_HEAD: u8 = 2;
const TAG_BODY: u8 = 3;
const TAG_END: u8 = 4;
const TAG_ERROR: u8 = 5;
const TAG_CANCEL: u8 = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Method {
    Get = 1,
    Head = 2,
}

/// Why the broker ended a request. The values are wire bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ErrorCode {
    /// The URL or a redirect target failed [`crate::policy`].
    PolicyRefused = 1,
    /// The name did not resolve.
    Dns = 2,
    /// The connection failed or was refused.
    Connect = 3,
    /// The TLS handshake or record layer failed.
    Tls = 4,
    /// The response was not valid HTTP/1.1.
    Http = 5,
    /// The response passed the broker's size ceiling.
    TooLarge = 6,
    /// No progress within the broker's timeout.
    Timeout = 7,
    /// More redirects than the limit.
    TooManyRedirects = 8,
    /// The content process has too many requests in flight.
    Busy = 9,
}

impl ErrorCode {
    fn from_byte(b: u8) -> Option<Self> {
        Some(match b {
            1 => Self::PolicyRefused,
            2 => Self::Dns,
            3 => Self::Connect,
            4 => Self::Tls,
            5 => Self::Http,
            6 => Self::TooLarge,
            7 => Self::Timeout,
            8 => Self::TooManyRedirects,
            9 => Self::Busy,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Msg {
    Fetch {
        id: u32,
        method: Method,
        url: String,
        headers: Vec<Header>,
    },
    Head {
        id: u32,
        status: u16,
        content_length: u32,
        headers: Vec<Header>,
        final_url: String,
    },
    Body {
        id: u32,
        data: Vec<u8>,
    },
    End {
        id: u32,
    },
    Error {
        id: u32,
        code: ErrorCode,
    },
    Cancel {
        id: u32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WireError {
    /// The message ended before a field did.
    Truncated,
    /// Bytes were left after the last field.
    Trailing,
    UnknownTag(u8),
    UnknownMethod(u8),
    UnknownError(u8),
    /// A length or count past its limit.
    Limit,
    /// Text that is not UTF-8.
    NotUtf8,
}

/// Encode `msg`. Refuses a message that [`decode`] would refuse for its
/// limits, so the broker can never send what the other end drops.
pub fn encode(msg: &Msg) -> Result<Vec<u8>, WireError> {
    let mut out = Vec::new();
    match msg {
        Msg::Fetch {
            id,
            method,
            url,
            headers,
        } => {
            out.push(TAG_FETCH);
            out.extend_from_slice(&id.to_le_bytes());
            out.push(*method as u8);
            put_str(&mut out, url, MAX_URL)?;
            put_headers(&mut out, headers)?;
        }
        Msg::Head {
            id,
            status,
            content_length,
            headers,
            final_url,
        } => {
            out.push(TAG_HEAD);
            out.extend_from_slice(&id.to_le_bytes());
            out.extend_from_slice(&status.to_le_bytes());
            out.extend_from_slice(&content_length.to_le_bytes());
            put_headers(&mut out, headers)?;
            put_str(&mut out, final_url, MAX_URL)?;
        }
        Msg::Body { id, data } => {
            if data.len() > MAX_BODY_CHUNK {
                return Err(WireError::Limit);
            }
            out.push(TAG_BODY);
            out.extend_from_slice(&id.to_le_bytes());
            out.extend_from_slice(&(data.len() as u16).to_le_bytes());
            out.extend_from_slice(data);
        }
        Msg::End { id } => {
            out.push(TAG_END);
            out.extend_from_slice(&id.to_le_bytes());
        }
        Msg::Error { id, code } => {
            out.push(TAG_ERROR);
            out.extend_from_slice(&id.to_le_bytes());
            out.push(*code as u8);
        }
        Msg::Cancel { id } => {
            out.push(TAG_CANCEL);
            out.extend_from_slice(&id.to_le_bytes());
        }
    }
    Ok(out)
}

fn put_str(out: &mut Vec<u8>, s: &str, max: usize) -> Result<(), WireError> {
    if s.len() > max {
        return Err(WireError::Limit);
    }
    out.extend_from_slice(&(s.len() as u16).to_le_bytes());
    out.extend_from_slice(s.as_bytes());
    Ok(())
}

fn put_headers(out: &mut Vec<u8>, headers: &[Header]) -> Result<(), WireError> {
    if headers.len() > MAX_HEADERS {
        return Err(WireError::Limit);
    }
    out.extend_from_slice(&(headers.len() as u16).to_le_bytes());
    for h in headers {
        put_str(out, &h.name, MAX_HEADER_NAME)?;
        put_str(out, &h.value, MAX_HEADER_VALUE)?;
    }
    Ok(())
}

struct Reader<'a> {
    buf: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], WireError> {
        let end = self.at.checked_add(n).ok_or(WireError::Truncated)?;
        let s = self.buf.get(self.at..end).ok_or(WireError::Truncated)?;
        self.at = end;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8, WireError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, WireError> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn u32(&mut self) -> Result<u32, WireError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn str(&mut self, max: usize) -> Result<String, WireError> {
        let len = self.u16()? as usize;
        if len > max {
            return Err(WireError::Limit);
        }
        let b = self.take(len)?;
        core::str::from_utf8(b)
            .map(String::from)
            .map_err(|_| WireError::NotUtf8)
    }

    fn headers(&mut self) -> Result<Vec<Header>, WireError> {
        let n = self.u16()? as usize;
        if n > MAX_HEADERS {
            return Err(WireError::Limit);
        }
        let mut v = Vec::with_capacity(n);
        for _ in 0..n {
            let name = self.str(MAX_HEADER_NAME)?;
            let value = self.str(MAX_HEADER_VALUE)?;
            v.push(Header { name, value });
        }
        Ok(v)
    }
}

/// Decode one message: the whole of `buf`, no more and no less.
pub fn decode(buf: &[u8]) -> Result<Msg, WireError> {
    let mut r = Reader { buf, at: 0 };
    let tag = r.u8()?;
    let id = r.u32()?;
    let msg = match tag {
        TAG_FETCH => {
            let method = match r.u8()? {
                1 => Method::Get,
                2 => Method::Head,
                m => return Err(WireError::UnknownMethod(m)),
            };
            let url = r.str(MAX_URL)?;
            let headers = r.headers()?;
            Msg::Fetch {
                id,
                method,
                url,
                headers,
            }
        }
        TAG_HEAD => {
            let status = r.u16()?;
            let content_length = r.u32()?;
            let headers = r.headers()?;
            let final_url = r.str(MAX_URL)?;
            Msg::Head {
                id,
                status,
                content_length,
                headers,
                final_url,
            }
        }
        TAG_BODY => {
            let len = r.u16()? as usize;
            if len > MAX_BODY_CHUNK {
                return Err(WireError::Limit);
            }
            Msg::Body {
                id,
                data: r.take(len)?.to_vec(),
            }
        }
        TAG_END => Msg::End { id },
        TAG_ERROR => {
            let b = r.u8()?;
            Msg::Error {
                id,
                code: ErrorCode::from_byte(b).ok_or(WireError::UnknownError(b))?,
            }
        }
        TAG_CANCEL => Msg::Cancel { id },
        t => return Err(WireError::UnknownTag(t)),
    };
    if r.at != buf.len() {
        return Err(WireError::Trailing);
    }
    Ok(msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;
    use alloc::vec;

    fn h(name: &str, value: &str) -> Header {
        Header {
            name: name.to_string(),
            value: value.to_string(),
        }
    }

    /// Byte oracle: each message written out field by field from THINK §2's
    /// table, not produced by `encode`.
    #[test]
    fn fetch_bytes() {
        let want: &[u8] = &[
            0x01, // tag FETCH
            0x07, 0x00, 0x00, 0x00, // id 7
            0x01, // GET
            0x0b, 0x00, // url_len 11
            b'h', b't', b't', b'p', b's', b':', b'/', b'/', b'a', b'.', b'b', // "https://a.b"
            0x01, 0x00, // 1 header
            0x01, 0x00, b'X', // name "X"
            0x02, 0x00, b'y', b'z', // value "yz"
        ];
        let msg = Msg::Fetch {
            id: 7,
            method: Method::Get,
            url: "https://a.b".to_string(),
            headers: vec![h("X", "yz")],
        };
        assert_eq!(encode(&msg).unwrap(), want);
        assert_eq!(decode(want).unwrap(), msg);
    }

    #[test]
    fn head_bytes() {
        let want: &[u8] = &[
            0x02, // tag HEAD
            0x2a, 0x00, 0x00, 0x00, // id 42
            0xc8, 0x00, // status 200
            0xff, 0xff, 0xff, 0xff, // content length unknown
            0x00, 0x00, // 0 headers
            0x03, 0x00, b'u', b'r', b'l', // final url "url"
        ];
        let msg = Msg::Head {
            id: 42,
            status: 200,
            content_length: LENGTH_UNKNOWN,
            headers: vec![],
            final_url: "url".to_string(),
        };
        assert_eq!(encode(&msg).unwrap(), want);
        assert_eq!(decode(want).unwrap(), msg);
    }

    #[test]
    fn small_messages_bytes() {
        let body: &[u8] = &[0x03, 0x01, 0x02, 0x03, 0x04, 0x02, 0x00, 0xaa, 0xbb];
        let body_msg = Msg::Body {
            id: 0x0403_0201,
            data: vec![0xaa, 0xbb],
        };
        assert_eq!(encode(&body_msg).unwrap(), body);
        assert_eq!(decode(body).unwrap(), body_msg);

        let end: &[u8] = &[0x04, 0xff, 0x00, 0x00, 0x00];
        assert_eq!(encode(&Msg::End { id: 255 }).unwrap(), end);
        assert_eq!(decode(end).unwrap(), Msg::End { id: 255 });

        let err: &[u8] = &[0x05, 0x01, 0x00, 0x00, 0x00, 0x06];
        let err_msg = Msg::Error {
            id: 1,
            code: ErrorCode::TooLarge,
        };
        assert_eq!(encode(&err_msg).unwrap(), err);
        assert_eq!(decode(err).unwrap(), err_msg);

        let cancel: &[u8] = &[0x06, 0x00, 0x00, 0x00, 0x80];
        assert_eq!(encode(&Msg::Cancel { id: 1 << 31 }).unwrap(), cancel);
        assert_eq!(decode(cancel).unwrap(), Msg::Cancel { id: 1 << 31 });
    }

    #[test]
    fn decode_refusals() {
        // Every proper prefix of a valid message is truncated.
        let full = encode(&Msg::Fetch {
            id: 1,
            method: Method::Head,
            url: "https://x".to_string(),
            headers: vec![h("a", "b")],
        })
        .unwrap();
        for n in 0..full.len() {
            assert_eq!(decode(&full[..n]), Err(WireError::Truncated), "prefix {n}");
        }
        let mut long = full.clone();
        long.push(0);
        assert_eq!(decode(&long), Err(WireError::Trailing));

        assert_eq!(decode(&[9, 0, 0, 0, 0]), Err(WireError::UnknownTag(9)));
        assert_eq!(
            decode(&[1, 0, 0, 0, 0, 3, 0, 0, 0, 0]),
            Err(WireError::UnknownMethod(3))
        );
        assert_eq!(decode(&[5, 0, 0, 0, 0, 0]), Err(WireError::UnknownError(0)));
        assert_eq!(
            decode(&[5, 0, 0, 0, 0, 10]),
            Err(WireError::UnknownError(10))
        );
        // A body length past the limit, before any payload is read.
        assert_eq!(decode(&[3, 0, 0, 0, 0, 0x01, 0x10]), Err(WireError::Limit));
        // A URL that is not UTF-8.
        assert_eq!(
            decode(&[1, 0, 0, 0, 0, 1, 1, 0, 0xff, 0, 0]),
            Err(WireError::NotUtf8)
        );
        // More headers than the limit.
        let mut many = vec![1, 0, 0, 0, 0, 1, 0, 0];
        many.extend_from_slice(&((MAX_HEADERS as u16) + 1).to_le_bytes());
        assert_eq!(decode(&many), Err(WireError::Limit));
    }

    #[test]
    fn encode_refuses_what_decode_would() {
        let url = "a".repeat(MAX_URL + 1);
        assert_eq!(
            encode(&Msg::Fetch {
                id: 0,
                method: Method::Get,
                url,
                headers: vec![],
            }),
            Err(WireError::Limit)
        );
        assert_eq!(
            encode(&Msg::Body {
                id: 0,
                data: vec![0; MAX_BODY_CHUNK + 1],
            }),
            Err(WireError::Limit)
        );
        // The limits themselves round-trip.
        let at_limit = Msg::Body {
            id: 0,
            data: vec![7; MAX_BODY_CHUNK],
        };
        assert_eq!(decode(&encode(&at_limit).unwrap()).unwrap(), at_limit);
    }
}
