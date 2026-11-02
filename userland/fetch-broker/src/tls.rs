// SPDX-License-Identifier: MPL-2.0
//! A TLS client stream with the record layer in user space (THINK §4).
//!
//! `libneo::tls` hands the session to the kernel after the handshake
//! (`sys_net_tcp_enable_tls`), and the kernel's record layer carries only
//! TLS 1.3 with ChaCha20-Poly1305. The broker talks to arbitrary servers, so it
//! keeps rustls' unbuffered connection for the whole session: the suites are
//! whatever the provider offers, TLS 1.2 and 1.3.

use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::time::Duration;

use libneo::{
    CLOCK_MONOTONIC, EAGAIN, TcpSocketHandle, sys_clock_gettime, sys_tcp_recv, sys_tcp_send,
};
use rustls::client::UnbufferedClientConnection;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::time_provider::TimeProvider;
use rustls::unbuffered::ConnectionState;
use rustls::{ClientConfig, RootCertStore};

const IO_BUF: usize = 16 * 1024 + 256;
/// The kernel's per-call limit on a socket send or receive
/// (`kernel/src/net/socket.rs` `MAX_DATAGRAM`); past it the call is EINVAL.
const SOCKET_IO_MAX: usize = 2048;
/// How long one socket wait may make no progress. The kernel never reports a
/// TCP close to `sys_tcp_recv` (an empty queue after FIN is EAGAIN), so a
/// server that goes away without close_notify is only noticed this way.
const IO_TIMEOUT_NS: u64 = 10_000_000_000;
/// Without a clock (a kernel without `browser-runtime`), polls instead.
const SPINS: usize = 1 << 16;

/// A wait that ends at a monotonic deadline, or after [`SPINS`] polls.
struct Wait {
    deadline: Option<u64>,
    spins: usize,
}

impl Wait {
    fn start() -> Self {
        Wait {
            deadline: sys_clock_gettime(CLOCK_MONOTONIC)
                .ok()
                .map(|t| t.saturating_add(IO_TIMEOUT_NS)),
            spins: 0,
        }
    }

    /// One more poll; `false` once the wait is over.
    fn again(&mut self) -> bool {
        self.spins += 1;
        match self.deadline {
            Some(d) => sys_clock_gettime(CLOCK_MONOTONIC).is_ok_and(|t| t < d),
            None => self.spins < SPINS,
        }
    }
}
/// The record-processing steps one call may take.
const STEPS: usize = 4096;

/// The clock certificate validity is checked against. This first build uses
/// the same fixed date as `libneo::tls` (the hermetic fixture's window); real
/// time from `CLOCK_REALTIME` on an `rtc-epoch` kernel is THINK §4's next item.
const FIXED_TIME_2026_05_30: u64 = 1_780_185_600;

#[derive(Debug)]
struct FixedTime;

impl TimeProvider for FixedTime {
    fn current_time(&self) -> Option<UnixTime> {
        Some(UnixTime::since_unix_epoch(Duration::from_secs(
            FIXED_TIME_2026_05_30,
        )))
    }
}

// The payloads are read through `Debug` (the broker logs them).
#[allow(dead_code)]
#[derive(Debug)]
pub enum TlsError {
    /// A socket call failed (the errno).
    Sys(i64),
    /// rustls refused the session (handshake, certificate, record).
    Rustls(rustls::Error),
    /// A connection state the stream does not expect at that point.
    Unexpected,
    /// The configuration or server name was rejected.
    Config,
    /// No progress within the stream's limits.
    Stalled,
}

pub struct TlsStream {
    socket: TcpSocketHandle,
    conn: UnbufferedClientConnection,
    /// Ciphertext received and not yet processed.
    incoming: Vec<u8>,
    /// Plaintext decrypted and not yet read.
    plain: Vec<u8>,
    /// The peer sent close_notify, or closed the TCP stream.
    closed: bool,
}

fn config(roots: &[&[u8]]) -> Result<ClientConfig, TlsError> {
    let mut store = RootCertStore::empty();
    for root in roots {
        store
            .add(CertificateDer::from(*root))
            .map_err(|_| TlsError::Config)?;
    }
    let provider = Arc::new(rustls_rustcrypto::provider());
    let mut config = ClientConfig::builder_with_details(provider, Arc::new(FixedTime))
        .with_protocol_versions(&[&rustls::version::TLS13, &rustls::version::TLS12])
        .map_err(|_| TlsError::Config)?
        .with_root_certificates(store)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(config)
}

impl TlsStream {
    /// Handshake on a connected socket, verifying `host` against `roots`.
    pub fn connect(socket: TcpSocketHandle, host: &str, roots: &[&[u8]]) -> Result<Self, TlsError> {
        let name = ServerName::try_from(host)
            .map_err(|_| TlsError::Config)?
            .to_owned();
        let conn = UnbufferedClientConnection::new(Arc::new(config(roots)?), name)
            .map_err(|_| TlsError::Config)?;
        let mut s = TlsStream {
            socket,
            conn,
            incoming: Vec::new(),
            plain: Vec::new(),
            closed: false,
        };
        // Drive until the session can carry application data.
        let mut out = vec![0u8; IO_BUF];
        let mut pending = Vec::new();
        for _ in 0..STEPS {
            let (discard, step) = {
                let status = s.conn.process_tls_records(s.incoming.as_mut_slice());
                let step = match status.state.map_err(TlsError::Rustls)? {
                    ConnectionState::EncodeTlsData(mut enc) => {
                        let n = enc.encode(&mut out).map_err(|_| TlsError::Unexpected)?;
                        pending.extend_from_slice(&out[..n]);
                        Step::Again
                    }
                    ConnectionState::TransmitTlsData(t) => {
                        send_all(s.socket, &pending)?;
                        pending.clear();
                        t.done();
                        Step::Again
                    }
                    ConnectionState::BlockedHandshake => Step::Read,
                    ConnectionState::WriteTraffic(_) => Step::Done,
                    _ => return Err(TlsError::Unexpected),
                };
                (status.discard, step)
            };
            discard_front(&mut s.incoming, discard);
            match step {
                Step::Done => return Ok(s),
                Step::Read => {
                    if !recv_append(s.socket, &mut s.incoming)? {
                        return Err(TlsError::Unexpected);
                    }
                }
                Step::Again => {}
            }
        }
        Err(TlsError::Stalled)
    }

    /// Encrypt and send all of `data`.
    pub fn write_all(&mut self, data: &[u8]) -> Result<(), TlsError> {
        let mut out = vec![0u8; IO_BUF];
        for chunk in data.chunks(8192) {
            let mut sent = false;
            for _ in 0..STEPS {
                let (discard, n) = {
                    let status = self.conn.process_tls_records(self.incoming.as_mut_slice());
                    let n = match status.state.map_err(TlsError::Rustls)? {
                        ConnectionState::WriteTraffic(mut w) => Some(
                            w.encrypt(chunk, &mut out)
                                .map_err(|_| TlsError::Unexpected)?,
                        ),
                        // Post-handshake records (a TLS 1.3 session ticket):
                        // read them into `plain` and come back.
                        ConnectionState::ReadTraffic(mut r) => {
                            while let Some(rec) = r.next_record() {
                                let rec = rec.map_err(TlsError::Rustls)?;
                                self.plain.extend_from_slice(rec.payload);
                            }
                            None
                        }
                        ConnectionState::BlockedHandshake => None,
                        _ => return Err(TlsError::Unexpected),
                    };
                    (status.discard, n)
                };
                discard_front(&mut self.incoming, discard);
                if let Some(n) = n {
                    send_all(self.socket, &out[..n])?;
                    sent = true;
                    break;
                }
            }
            if !sent {
                return Err(TlsError::Stalled);
            }
        }
        Ok(())
    }

    /// Read decrypted bytes into `buf`; 0 means the peer closed the stream.
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, TlsError> {
        for _ in 0..STEPS {
            if !self.plain.is_empty() {
                let n = buf.len().min(self.plain.len());
                buf[..n].copy_from_slice(&self.plain[..n]);
                self.plain.drain(..n);
                return Ok(n);
            }
            if self.closed {
                return Ok(0);
            }
            let (discard, need) = {
                let status = self.conn.process_tls_records(self.incoming.as_mut_slice());
                let need = match status.state.map_err(TlsError::Rustls)? {
                    ConnectionState::ReadTraffic(mut r) => {
                        while let Some(rec) = r.next_record() {
                            let rec = rec.map_err(TlsError::Rustls)?;
                            self.plain.extend_from_slice(rec.payload);
                        }
                        false
                    }
                    ConnectionState::PeerClosed | ConnectionState::Closed => {
                        self.closed = true;
                        false
                    }
                    ConnectionState::WriteTraffic(_) | ConnectionState::BlockedHandshake => true,
                    _ => return Err(TlsError::Unexpected),
                };
                (status.discard, need)
            };
            discard_front(&mut self.incoming, discard);
            if need && !recv_append(self.socket, &mut self.incoming)? {
                // TCP closed without close_notify: whatever was framed is all
                // there is; the HTTP layer decides whether that's truncation.
                self.closed = true;
            }
        }
        Err(TlsError::Stalled)
    }
}

enum Step {
    Again,
    Read,
    Done,
}

fn send_all(socket: TcpSocketHandle, mut buf: &[u8]) -> Result<(), TlsError> {
    let mut wait = Wait::start();
    while !buf.is_empty() {
        let n = sys_tcp_send(socket, &buf[..buf.len().min(SOCKET_IO_MAX)]);
        if n > 0 {
            buf = &buf[n as usize..];
            wait = Wait::start();
        } else if n == EAGAIN {
            if !wait.again() {
                return Err(TlsError::Stalled);
            }
            core::hint::spin_loop();
        } else {
            return Err(TlsError::Sys(n));
        }
    }
    Ok(())
}

/// Append what the socket has; `false` when the peer closed it.
fn recv_append(socket: TcpSocketHandle, incoming: &mut Vec<u8>) -> Result<bool, TlsError> {
    let mut buf = [0u8; SOCKET_IO_MAX];
    let mut wait = Wait::start();
    loop {
        let n = sys_tcp_recv(socket, &mut buf);
        if n > 0 {
            incoming.extend_from_slice(&buf[..n as usize]);
            return Ok(true);
        }
        if n == 0 {
            return Ok(false);
        }
        if n != EAGAIN {
            return Err(TlsError::Sys(n));
        }
        if !wait.again() {
            return Err(TlsError::Stalled);
        }
        core::hint::spin_loop();
    }
}

fn discard_front(buf: &mut Vec<u8>, n: usize) {
    if n >= buf.len() {
        buf.clear();
    } else if n > 0 {
        buf.drain(..n);
    }
}
