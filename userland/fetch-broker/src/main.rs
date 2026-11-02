// SPDX-License-Identifier: MPL-2.0
//! OT-61 rung 5: the fetch broker (`docs/THINK-2026-10-30-ot61-r5-fetch-broker.md`).
//!
//! The one process in the browser bundle that holds the dial cap. Its
//! launcher hands it, at spawn:
//! - handle 0: the channel to one content process;
//! - handle 1: the root `NetDialCap`.
//!
//! The content process sends `FETCH` messages (`fetch_proto::wire`); for each
//! one the broker checks the URL (`fetch_proto::policy`), derives a cap for
//! exactly that host and port with `NON_PUBLIC` narrowed away (so the kernel
//! refuses a name that resolves to a private address, rung 5b), dials, runs
//! TLS in user space ([`tls`]), sends `GET`, follows up to
//! [`MAX_REDIRECTS`] redirects (each hop re-checked), and streams the response
//! back as `HEAD`, `BODY`… and `END`, or `ERROR`. Every derived cap is revoked
//! when its connection ends. Requests are served one at a time.

#![no_std]
#![no_main]

extern crate alloc;

mod tls;

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use fetch_proto::chunked::Chunked;
use fetch_proto::http::{Framing, Head, parse_head};
use fetch_proto::policy::{self, Target};
use fetch_proto::wire::{self, ErrorCode, Header, Method, Msg};
use libneo::{
    CapRights, EACCES, EAGAIN, EBADF, EPERM, FileHandle, Handle, NetDialCap, meta_rights,
    net_dial_rights, sys_close, sys_derive, sys_exit, sys_grant, sys_net_dial,
    sys_net_dial_cap_derive, sys_recv, sys_revoke, sys_send, sys_tcp_close, sys_write,
};

use tls::TlsStream;

const CHANNEL: Handle = Handle(0);
const ROOT_DIAL: NetDialCap = NetDialCap(Handle(1));

/// The Fetch standard's redirect limit.
const MAX_REDIRECTS: usize = 20;
/// The largest response body the broker forwards.
const MAX_BODY: usize = 16 * 1024 * 1024;
/// One byte past the longest message, so an over-long send is refused whole
/// instead of being cut to a prefix that decodes (`wire::MAX_MESSAGE`).
const RECV_BUF: usize = wire::RECV_BUF;

/// The trust anchors. This first build serves the hermetic fixture only;
/// the Mozilla store (THINK §4) replaces this list.
const ROOTS: &[&[u8]] = libneo::tls::PHASE4_TEST_ROOTS;

pub(crate) fn say(msg: &str) {
    let _ = sys_write(1, msg.as_ptr(), msg.len());
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    say("[ fetch-broker ] up\n");
    let requests = request_root();
    let mut buf = vec![0u8; RECV_BUF];
    loop {
        let n = sys_recv(CHANNEL, &mut buf);
        if n < 0 {
            // The content process is gone: nothing left to serve.
            say("[ fetch-broker ] channel closed, exiting\n");
            sys_exit(0);
        }
        match wire::decode(&buf[..n as usize]) {
            Ok(Msg::Fetch {
                id, method, url, ..
            }) => serve(id, method, url, requests),
            // Requests are served one at a time, so a CANCEL can only name
            // one that has already ended.
            Ok(Msg::Cancel { .. }) => {}
            // Anything else from content is a protocol error: drop it.
            Ok(_) | Err(_) => say("[ fetch-broker ] dropped a malformed message\n"),
        }
    }
}

fn send(msg: &Msg) {
    if let Ok(bytes) = wire::encode(msg) {
        let _ = sys_send(CHANNEL, &bytes);
    }
}

fn fail(id: u32, code: ErrorCode) {
    send(&Msg::Error { id, code });
}

/// The cap every request's scope is derived from: the root's dial rights
/// without `GRANT` but with `REVOKE` (#29 review item 4 and its re-check).
///
/// The launcher's root (`ROOT_DIAL`) carries `GRANT`, and scope derivation
/// copies its parent's rights, so a scope taken straight from it could be
/// spawned or granted onward and outlive this process. So the broker derives
/// this cap, then closes the root: from here on it holds no dial cap with
/// `GRANT`, nothing it derives can leave it, and every dial cap it has is in
/// its own table, which its teardown drains. The derived cap re-parents to the
/// launcher's root, so the launcher's revoke still reaches it.
///
/// `REVOKE` is kept so each request's scope (which inherits these rights) can
/// be revoked when the request ends; without it `Conn::close`'s revoke fails
/// with EPERM and scopes pile up. `NON_PUBLIC` is kept when the root has it
/// (a `net-dial-public-only` kernel); the per-request leaf drops it again
/// (rung 5b). Without this cap the broker can't serve anything.
fn request_root() -> NetDialCap {
    let meta = meta_rights::USE | meta_rights::DERIVE | meta_rights::REVOKE;
    let wide = CapRights::new(meta, net_dial_rights::DIAL | net_dial_rights::NON_PUBLIC);
    let narrow = CapRights::new(meta, net_dial_rights::DIAL);
    let requests = match sys_derive(ROOT_DIAL.0, wide).or_else(|_| sys_derive(ROOT_DIAL.0, narrow))
    {
        Ok(h) => NetDialCap(h),
        Err(_) => {
            say("[ fetch-broker ] no request cap from the root dial cap\n");
            sys_exit(2);
        }
    };
    if sys_close(FileHandle(ROOT_DIAL.0.0)) != 0 {
        say("[ fetch-broker ] could not close the root dial cap\n");
        sys_exit(2);
    }
    // Self-check, on the serial oracle: the root is gone, and the cap the
    // broker keeps can't be handed on.
    let root_gone = sys_derive(ROOT_DIAL.0, narrow) == Err(EBADF);
    let grant_refused = sys_grant(
        CHANNEL,
        requests.0,
        CapRights::new(meta_rights::USE, net_dial_rights::DIAL),
    ) == EPERM;
    say(&format!(
        "[ fetch-broker ] root dial cap closed: derive_from_root={} grant_request_cap={}\n",
        if root_gone { "refused" } else { "ALLOWED" },
        if grant_refused { "refused" } else { "ALLOWED" },
    ));
    requests
}

/// Revoke a request's scope (and the narrowed leaf under it). A failure is
/// said on serial, never ignored: a scope left installed is a leak.
fn revoke_scope(scope: NetDialCap) {
    let r = sys_revoke(scope.0);
    if r == 0 {
        say("[ fetch-broker ] request scope revoked\n");
    } else {
        say(&format!(
            "[ fetch-broker ] revoke of a request scope failed: {r}\n"
        ));
    }
}

/// One request, start to finish (redirects included).
fn serve(id: u32, method: Method, url: String, requests: NetDialCap) {
    let mut url = url;
    for _ in 0..=MAX_REDIRECTS {
        let target = match policy::check_url(&url) {
            Ok(t) => t,
            Err(_) => return fail(id, ErrorCode::PolicyRefused),
        };
        let mut conn = match Conn::open(&target, requests) {
            Ok(c) => c,
            Err(code) => return fail(id, code),
        };
        let result = conn.exchange(id, method, &target);
        conn.close();
        match result {
            Ok(Some(next)) => url = next,
            Ok(None) => return,
            Err(code) => return fail(id, code),
        }
    }
    fail(id, ErrorCode::TooManyRedirects)
}

/// One connection: its derived caps, socket and TLS session.
struct Conn {
    scope: NetDialCap,
    socket: libneo::TcpSocketHandle,
    tls: Option<TlsStream>,
}

impl Conn {
    fn open(target: &Target, requests: NetDialCap) -> Result<Conn, ErrorCode> {
        let scope = sys_net_dial_cap_derive(requests, &target.host, target.port)
            .map_err(|_| ErrorCode::PolicyRefused)?;
        let dial = if keep_non_public(&target.host) {
            scope
        } else {
            // The per-request cap reaches only public addresses (rung 5b).
            match sys_derive(
                scope.0,
                CapRights::new(meta_rights::USE, net_dial_rights::DIAL),
            ) {
                Ok(h) => NetDialCap(h),
                Err(_) => {
                    revoke_scope(scope);
                    return Err(ErrorCode::PolicyRefused);
                }
            }
        };
        let socket = loop {
            match sys_net_dial(dial, &target.host, target.port) {
                Ok(s) => break s,
                Err(EAGAIN) => core::hint::spin_loop(),
                Err(e) => {
                    // Revoking the scope revokes the narrowed child too.
                    revoke_scope(scope);
                    return Err(if e == EACCES {
                        ErrorCode::PolicyRefused
                    } else {
                        ErrorCode::Connect
                    });
                }
            }
        };
        let mut conn = Conn {
            scope,
            socket,
            tls: None,
        };
        match TlsStream::connect(socket, &target.host, ROOTS) {
            Ok(t) => conn.tls = Some(t),
            Err(e) => {
                say(&format!(
                    "[ fetch-broker ] tls handshake with {}: {e:?}\n",
                    target.host
                ));
                conn.close();
                return Err(ErrorCode::Tls);
            }
        }
        Ok(conn)
    }

    fn close(&mut self) {
        self.tls = None;
        let _ = sys_tcp_close(self.socket);
        revoke_scope(self.scope);
    }

    /// Send the request and forward the response. `Ok(Some(url))`: a redirect
    /// to follow.
    fn exchange(
        &mut self,
        id: u32,
        method: Method,
        target: &Target,
    ) -> Result<Option<String>, ErrorCode> {
        let tls = self.tls.as_mut().ok_or(ErrorCode::Tls)?;
        let verb = match method {
            Method::Get => "GET",
            Method::Head => "HEAD",
        };
        let host = authority(target);
        let request = format!(
            "{verb} {} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\nAccept: */*\r\n\
             Accept-Encoding: identity\r\nUser-Agent: LogansOS-fetch/0\r\n\r\n",
            target.path
        );
        tls.write_all(request.as_bytes())
            .map_err(|e| tls_failed("write", e))?;

        // The head.
        let mut raw: Vec<u8> = Vec::new();
        let mut chunk = vec![0u8; 16 * 1024];
        let (head, body_start) = loop {
            match parse_head(&raw, method == Method::Head) {
                Ok(Some(found)) => break found,
                Ok(None) => {}
                Err(_) => return Err(ErrorCode::Http),
            }
            let n = tls.read(&mut chunk).map_err(|e| tls_failed("read", e))?;
            if n == 0 {
                return Err(ErrorCode::Http);
            }
            raw.extend_from_slice(&chunk[..n]);
        };
        if head.is_redirect()
            && let Some(location) = head.get("location")
        {
            return policy::resolve_location(target, location)
                .map(Some)
                .map_err(|_| ErrorCode::PolicyRefused);
        }
        send(&head_msg(id, &head, target));

        // The body, as its framing says.
        let mut early = raw.split_off(body_start);
        let mut forwarded = 0usize;
        let mut forward = |data: &[u8]| -> Result<(), ErrorCode> {
            forwarded += data.len();
            if forwarded > MAX_BODY {
                return Err(ErrorCode::TooLarge);
            }
            for piece in data.chunks(wire::MAX_BODY_CHUNK) {
                send(&Msg::Body {
                    id,
                    data: piece.to_vec(),
                });
            }
            Ok(())
        };
        match head.framing {
            Framing::None => {}
            Framing::Length(len) => {
                if len > MAX_BODY as u64 {
                    return Err(ErrorCode::TooLarge);
                }
                let mut left = len as usize;
                let take = left.min(early.len());
                forward(&early[..take])?;
                left -= take;
                while left > 0 {
                    let n = tls.read(&mut chunk).map_err(|e| tls_failed("read", e))?;
                    if n == 0 {
                        return Err(ErrorCode::Http); // truncated
                    }
                    let take = left.min(n);
                    forward(&chunk[..take])?;
                    left -= take;
                }
            }
            Framing::Chunked => {
                let mut dec = Chunked::new(MAX_BODY);
                let mut out = Vec::new();
                let mut input = core::mem::take(&mut early);
                loop {
                    let (_, done) = dec.feed(&input, &mut out).map_err(|e| match e {
                        fetch_proto::chunked::ChunkError::TooLarge => ErrorCode::TooLarge,
                        _ => ErrorCode::Http,
                    })?;
                    forward(&out)?;
                    out.clear();
                    if done {
                        break;
                    }
                    let n = tls.read(&mut chunk).map_err(|e| tls_failed("read", e))?;
                    if n == 0 {
                        return Err(ErrorCode::Http);
                    }
                    input = chunk[..n].to_vec();
                }
            }
            Framing::Close => {
                forward(&early)?;
                loop {
                    let n = tls.read(&mut chunk).map_err(|e| tls_failed("read", e))?;
                    if n == 0 {
                        break;
                    }
                    forward(&chunk[..n])?;
                }
            }
        }
        send(&Msg::End { id });
        Ok(None)
    }
}

fn tls_failed(what: &str, e: tls::TlsError) -> ErrorCode {
    say(&format!("[ fetch-broker ] tls {what}: {e:?}\n"));
    ErrorCode::Tls
}

/// `Host:` as the URL had it (the port only when it isn't 443).
fn authority(t: &Target) -> String {
    let host = if t.host.contains(':') {
        format!("[{}]", t.host)
    } else {
        t.host.clone()
    };
    if t.port == 443 {
        host
    } else {
        format!("{host}:{}", t.port)
    }
}

/// The `HEAD` message: status, length, the fields content may see, and the
/// final URL. `Set-Cookie` stays in the broker (cookies are the broker's job,
/// THINK §5), as do hop-by-hop fields.
fn head_msg(id: u32, head: &Head, target: &Target) -> Msg {
    const HIDDEN: [&str; 6] = [
        "set-cookie",
        "set-cookie2",
        "connection",
        "keep-alive",
        "transfer-encoding",
        "proxy-authenticate",
    ];
    let headers: Vec<Header> = head
        .headers
        .iter()
        .filter(|h| !HIDDEN.iter().any(|x| h.name.eq_ignore_ascii_case(x)))
        // A field past the wire's own per-field limits can't be sent at all.
        .filter(|h| {
            h.name.len() <= wire::MAX_HEADER_NAME && h.value.len() <= wire::MAX_HEADER_VALUE
        })
        .take(wire::MAX_HEADERS)
        .cloned()
        .collect();
    let content_length = match head.framing {
        Framing::Length(n) => u32::try_from(n).unwrap_or(wire::LENGTH_UNKNOWN),
        Framing::None => 0,
        _ => wire::LENGTH_UNKNOWN,
    };
    // The whole message must fit `wire::MAX_MESSAGE`; fields past that are
    // dropped from the end.
    let mut headers = headers;
    loop {
        let msg = Msg::Head {
            id,
            status: head.status,
            content_length,
            headers: headers.clone(),
            final_url: target.url(),
        };
        if wire::encode(&msg).is_ok() || headers.pop().is_none() {
            return msg;
        }
    }
}

#[cfg(feature = "fixture-loopback")]
fn keep_non_public(host: &str) -> bool {
    host == libneo::tls::PHASE4_TEST_HOST
}

#[cfg(not(feature = "fixture-loopback"))]
fn keep_non_public(_host: &str) -> bool {
    false
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    say("[ fetch-broker ] panic\n");
    sys_exit(101);
}
