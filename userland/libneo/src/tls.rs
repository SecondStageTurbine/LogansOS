// SPDX-License-Identifier: MPL-2.0
//! TLS 1.3 client/server helpers backed by rustls.

extern crate alloc;

use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::time::Duration;

use rustls::client::UnbufferedClientConnection;
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer, ServerName, UnixTime};
use rustls::server::UnbufferedServerConnection;
use rustls::time_provider::TimeProvider;
use rustls::unbuffered::ConnectionState;
use rustls::{ClientConfig, ConnectionTrafficSecrets, RootCertStore, ServerConfig};

use crate::{
    EAGAIN, TLS_KEY_MATERIAL_LEN, TcpSocketHandle, sys_net_tcp_enable_tls, sys_tcp_recv,
    sys_tcp_send,
};

pub const PHASE4_TEST_HOST: &str = "phase4.test";
pub const PHASE4_TEST_PORT: u16 = 4433;
pub const PHASE4_TEST_BODY: &[u8] = b"phase4 https body\n";
pub const PHASE4_TEST_ROOTS: &[&[u8]] = &[include_bytes!("../fixtures/phase4.test.root.der")];
pub const PHASE4_TEST_CERT_CHAIN: &[&[u8]] = &[
    include_bytes!("../fixtures/phase4.test.cert.der"),
    include_bytes!("../fixtures/phase4.test.root.der"),
];
pub const PHASE4_TEST_KEY_PKCS8: &[u8] = include_bytes!("../fixtures/phase4.test.key.pkcs8.der");

const TLS_IO_BUF: usize = 2048;
const TLS_STEPS: usize = 512;
const HTTP_STEPS: usize = 128;
const FIXED_TIME_2026_05_30: u64 = 1_780_185_600;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum TlsError {
    Sys(i64),
    Rustls,
    RustlsAlert,
    RustlsCertBadSig,
    RustlsCertExpired,
    RustlsCertName,
    RustlsCertNotYet,
    RustlsCertPurpose,
    RustlsCertSigAlg,
    RustlsCertUnknownIssuer,
    RustlsCert,
    RustlsDecrypt,
    RustlsInvalidMessage,
    RustlsPeer,
    RustlsRandom,
    RustlsConfig,
    RustlsNew,
    RustlsProcess,
    RustlsEncode,
    RustlsExtract,
    InvalidName,
    UnsupportedSuite,
    NonZeroSequence,
    BufferTooSmall,
    Truncated,
    BadHttp,
    Limit,
}

impl TlsError {
    pub const fn code(self) -> u8 {
        match self {
            TlsError::Sys(_) => b'S',
            TlsError::Rustls => b'R',
            TlsError::RustlsAlert => b'A',
            TlsError::RustlsCertBadSig => b'K',
            TlsError::RustlsCertExpired => b'e',
            TlsError::RustlsCertName => b'n',
            TlsError::RustlsCertNotYet => b't',
            TlsError::RustlsCertPurpose => b'p',
            TlsError::RustlsCertSigAlg => b'I',
            TlsError::RustlsCertUnknownIssuer => b'Z',
            TlsError::RustlsCert => b'V',
            TlsError::RustlsDecrypt => b'D',
            TlsError::RustlsInvalidMessage => b'M',
            TlsError::RustlsPeer => b'Y',
            TlsError::RustlsRandom => b'G',
            TlsError::RustlsConfig => b'C',
            TlsError::RustlsNew => b'c',
            TlsError::RustlsProcess => b'P',
            TlsError::RustlsEncode => b'E',
            TlsError::RustlsExtract => b'X',
            TlsError::InvalidName => b'N',
            TlsError::UnsupportedSuite => b'U',
            TlsError::NonZeroSequence => b'Q',
            TlsError::BufferTooSmall => b'B',
            TlsError::Truncated => b'T',
            TlsError::BadHttp => b'H',
            TlsError::Limit => b'L',
        }
    }
}

#[derive(Debug)]
struct FixedTime;

impl TimeProvider for FixedTime {
    fn current_time(&self) -> Option<UnixTime> {
        Some(UnixTime::since_unix_epoch(Duration::from_secs(
            FIXED_TIME_2026_05_30,
        )))
    }
}

pub fn https_get(
    socket: TcpSocketHandle,
    host: &str,
    path: &str,
    roots: &[&[u8]],
    body_out: &mut [u8],
) -> Result<usize, TlsError> {
    tls13_client_handshake(socket, host, roots)?;

    let mut request = Vec::with_capacity(path.len() + host.len() + 96);
    request.extend_from_slice(b"GET ");
    request.extend_from_slice(path.as_bytes());
    request.extend_from_slice(b" HTTP/1.1\r\nHost: ");
    request.extend_from_slice(host.as_bytes());
    request.extend_from_slice(b"\r\nConnection: close\r\nAccept: */*\r\n\r\n");
    send_all(socket, &request)?;

    let mut response = Vec::new();
    let mut buf = vec![0u8; 1024];
    for _ in 0..HTTP_STEPS {
        let n = sys_tcp_recv(socket, &mut buf);
        if n > 0 {
            response.extend_from_slice(&buf[..n as usize]);
            if let Some(body_start) = find_bytes(&response, b"\r\n\r\n") {
                let body_start = body_start + 4;
                let content_len = content_length(&response[..body_start])?;
                if response.len().saturating_sub(body_start) >= content_len {
                    if content_len > body_out.len() {
                        return Err(TlsError::BufferTooSmall);
                    }
                    body_out[..content_len]
                        .copy_from_slice(&response[body_start..body_start + content_len]);
                    return Ok(content_len);
                }
            }
        } else if n == EAGAIN {
            core::hint::spin_loop();
        } else {
            return Err(TlsError::Sys(n));
        }
    }
    Err(TlsError::Truncated)
}

pub fn tls13_client_handshake(
    socket: TcpSocketHandle,
    host: &str,
    roots: &[&[u8]],
) -> Result<(), TlsError> {
    let config = client_config(roots)?;
    let server_name = ServerName::try_from(host)
        .map_err(|_| TlsError::InvalidName)?
        .to_owned();
    let mut conn = UnbufferedClientConnection::new(Arc::new(config), server_name)
        .map_err(|_| TlsError::RustlsNew)?;
    drive_handshake_client(socket, &mut conn)?;
    let (secrets, _kernel_conn) = conn
        .dangerous_into_kernel_connection()
        .map_err(|_| TlsError::RustlsExtract)?;
    let mut key_material = [0u8; TLS_KEY_MATERIAL_LEN];
    encode_key_material(&secrets, &mut key_material)?;
    let ret = sys_net_tcp_enable_tls(socket, &mut key_material);
    key_material.fill(0);
    if ret < 0 {
        Err(TlsError::Sys(ret))
    } else {
        Ok(())
    }
}

pub fn serve_https_once(
    socket: TcpSocketHandle,
    cert_chain: &[&[u8]],
    key_pkcs8: &[u8],
    body: &[u8],
) -> Result<(), TlsError> {
    let config = server_config(cert_chain, key_pkcs8)?;
    let mut conn =
        UnbufferedServerConnection::new(Arc::new(config)).map_err(|_| TlsError::RustlsNew)?;
    drive_handshake_server(socket, &mut conn)?;
    let request = server_read_request(socket, &mut conn)?;
    if !request.starts_with(b"GET /") {
        return Err(TlsError::BadHttp);
    }
    server_write_response(socket, &mut conn, body)
}

fn provider() -> Arc<CryptoProvider> {
    let mut provider = rustls_rustcrypto::provider();
    provider.cipher_suites = vec![rustls_rustcrypto::TLS13_CHACHA20_POLY1305_SHA256];
    Arc::new(provider)
}

fn client_config(roots: &[&[u8]]) -> Result<ClientConfig, TlsError> {
    let mut root_store = RootCertStore::empty();
    for root in roots {
        root_store
            .add(CertificateDer::from(*root))
            .map_err(|_| TlsError::RustlsConfig)?;
    }
    let mut config = ClientConfig::builder_with_details(provider(), Arc::new(FixedTime))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|_| TlsError::RustlsConfig)?
        .with_root_certificates(root_store)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    config.enable_secret_extraction = true;
    Ok(config)
}

fn server_config(cert_chain: &[&[u8]], key_pkcs8: &[u8]) -> Result<ServerConfig, TlsError> {
    let certs = cert_chain
        .iter()
        .map(|cert| CertificateDer::from(*cert).into_owned())
        .collect();
    let key = PrivateKeyDer::from(PrivatePkcs8KeyDer::from(key_pkcs8).clone_key());
    let mut config = ServerConfig::builder_with_details(provider(), Arc::new(FixedTime))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|_| TlsError::RustlsConfig)?
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|_| TlsError::RustlsConfig)?;
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(config)
}

fn drive_handshake_client(
    socket: TcpSocketHandle,
    conn: &mut UnbufferedClientConnection,
) -> Result<(), TlsError> {
    let mut incoming = Vec::new();
    let mut pending = Vec::new();
    let mut out = vec![0u8; TLS_IO_BUF];
    for _ in 0..TLS_STEPS {
        let mut need_read = false;
        let mut complete = false;
        let discard;
        {
            let status = conn.process_tls_records(incoming.as_mut_slice());
            discard = status.discard;
            match status.state.map_err(map_rustls_error)? {
                ConnectionState::EncodeTlsData(mut encoder) => {
                    let n = encoder
                        .encode(&mut out)
                        .map_err(|_| TlsError::RustlsEncode)?;
                    pending.extend_from_slice(&out[..n]);
                }
                ConnectionState::TransmitTlsData(transmit) => {
                    send_all(socket, &pending)?;
                    pending.clear();
                    transmit.done();
                }
                ConnectionState::BlockedHandshake => need_read = true,
                ConnectionState::WriteTraffic(_) => complete = true,
                ConnectionState::PeerClosed | ConnectionState::Closed => {
                    return Err(TlsError::Rustls);
                }
                ConnectionState::ReadTraffic(_) | ConnectionState::ReadEarlyData(_) => {
                    return Err(TlsError::Rustls);
                }
                _ => return Err(TlsError::Rustls),
            }
        }
        discard_front(&mut incoming, discard);
        if complete {
            return Ok(());
        }
        if need_read {
            recv_append(socket, &mut incoming)?;
        }
    }
    Err(TlsError::Limit)
}

fn drive_handshake_server(
    socket: TcpSocketHandle,
    conn: &mut UnbufferedServerConnection,
) -> Result<(), TlsError> {
    let mut incoming = Vec::new();
    let mut pending = Vec::new();
    let mut out = vec![0u8; TLS_IO_BUF];
    for _ in 0..TLS_STEPS {
        let mut need_read = false;
        let mut complete = false;
        let discard;
        {
            let status = conn.process_tls_records(incoming.as_mut_slice());
            discard = status.discard;
            match status.state.map_err(map_rustls_error)? {
                ConnectionState::EncodeTlsData(mut encoder) => {
                    let n = encoder
                        .encode(&mut out)
                        .map_err(|_| TlsError::RustlsEncode)?;
                    pending.extend_from_slice(&out[..n]);
                }
                ConnectionState::TransmitTlsData(transmit) => {
                    send_all(socket, &pending)?;
                    pending.clear();
                    transmit.done();
                }
                ConnectionState::BlockedHandshake => need_read = true,
                ConnectionState::WriteTraffic(_) => complete = true,
                ConnectionState::PeerClosed | ConnectionState::Closed => {
                    return Err(TlsError::Rustls);
                }
                ConnectionState::ReadTraffic(_) | ConnectionState::ReadEarlyData(_) => {
                    return Err(TlsError::Rustls);
                }
                _ => return Err(TlsError::Rustls),
            }
        }
        discard_front(&mut incoming, discard);
        if complete {
            return Ok(());
        }
        if need_read {
            recv_append(socket, &mut incoming)?;
        }
    }
    Err(TlsError::Limit)
}

fn server_read_request(
    socket: TcpSocketHandle,
    conn: &mut UnbufferedServerConnection,
) -> Result<Vec<u8>, TlsError> {
    let mut incoming = Vec::new();
    let mut request = Vec::new();
    for _ in 0..HTTP_STEPS {
        recv_append(socket, &mut incoming)?;
        let discard;
        {
            let status = conn.process_tls_records(incoming.as_mut_slice());
            discard = status.discard;
            match status.state.map_err(map_rustls_error)? {
                ConnectionState::ReadTraffic(mut reader) => {
                    while let Some(record) = reader.next_record() {
                        let record = record.map_err(|_| TlsError::Rustls)?;
                        request.extend_from_slice(record.payload);
                    }
                }
                ConnectionState::WriteTraffic(_) | ConnectionState::BlockedHandshake => {}
                ConnectionState::EncodeTlsData(_) | ConnectionState::TransmitTlsData(_) => {
                    return Err(TlsError::Rustls);
                }
                ConnectionState::PeerClosed | ConnectionState::Closed => {
                    return Err(TlsError::Rustls);
                }
                ConnectionState::ReadEarlyData(_) => return Err(TlsError::Rustls),
                _ => return Err(TlsError::Rustls),
            }
        }
        discard_front(&mut incoming, discard);
        if find_bytes(&request, b"\r\n\r\n").is_some() {
            return Ok(request);
        }
    }
    Err(TlsError::Truncated)
}

fn server_write_response(
    socket: TcpSocketHandle,
    conn: &mut UnbufferedServerConnection,
    body: &[u8],
) -> Result<(), TlsError> {
    let mut response = Vec::with_capacity(body.len() + 96);
    response.extend_from_slice(b"HTTP/1.1 200 OK\r\nContent-Length: ");
    push_decimal(body.len(), &mut response);
    response.extend_from_slice(b"\r\nConnection: close\r\n\r\n");
    response.extend_from_slice(body);

    let mut incoming = Vec::new();
    let mut out = vec![0u8; TLS_IO_BUF];
    for _ in 0..HTTP_STEPS {
        let discard;
        let mut written = 0usize;
        {
            let status = conn.process_tls_records(incoming.as_mut_slice());
            discard = status.discard;
            match status.state.map_err(map_rustls_error)? {
                ConnectionState::WriteTraffic(mut writer) => {
                    written = writer
                        .encrypt(&response, &mut out)
                        .map_err(|_| TlsError::BufferTooSmall)?;
                }
                ConnectionState::BlockedHandshake => recv_append(socket, &mut incoming)?,
                ConnectionState::EncodeTlsData(_) | ConnectionState::TransmitTlsData(_) => {
                    return Err(TlsError::Rustls);
                }
                ConnectionState::ReadTraffic(_)
                | ConnectionState::ReadEarlyData(_)
                | ConnectionState::PeerClosed
                | ConnectionState::Closed => return Err(TlsError::Rustls),
                _ => return Err(TlsError::Rustls),
            }
        }
        discard_front(&mut incoming, discard);
        if written > 0 {
            return send_all(socket, &out[..written]);
        }
    }
    Err(TlsError::Limit)
}

fn encode_key_material(
    secrets: &rustls::ExtractedSecrets,
    out: &mut [u8; TLS_KEY_MATERIAL_LEN],
) -> Result<(), TlsError> {
    if secrets.tx.0 != 0 || secrets.rx.0 != 0 {
        return Err(TlsError::NonZeroSequence);
    }
    copy_secret(&secrets.tx.1, &mut out[0..44])?;
    copy_secret(&secrets.rx.1, &mut out[44..88])?;
    Ok(())
}

fn copy_secret(secret: &ConnectionTrafficSecrets, out: &mut [u8]) -> Result<(), TlsError> {
    match secret {
        ConnectionTrafficSecrets::Chacha20Poly1305 { key, iv } => {
            if out.len() != 44 || key.as_ref().len() != 32 || iv.as_ref().len() != 12 {
                return Err(TlsError::UnsupportedSuite);
            }
            out[..32].copy_from_slice(key.as_ref());
            out[32..44].copy_from_slice(iv.as_ref());
            Ok(())
        }
        _ => Err(TlsError::UnsupportedSuite),
    }
}

fn map_rustls_error(err: rustls::Error) -> TlsError {
    match err {
        rustls::Error::AlertReceived(_) => TlsError::RustlsAlert,
        rustls::Error::DecryptError => TlsError::RustlsDecrypt,
        rustls::Error::FailedToGetRandomBytes => TlsError::RustlsRandom,
        rustls::Error::InvalidCertificate(err) => map_certificate_error(err),
        rustls::Error::InvalidMessage(_) => TlsError::RustlsInvalidMessage,
        rustls::Error::PeerIncompatible(_) | rustls::Error::PeerMisbehaved(_) => {
            TlsError::RustlsPeer
        }
        _ => TlsError::RustlsProcess,
    }
}

fn map_certificate_error(err: rustls::CertificateError) -> TlsError {
    match err {
        rustls::CertificateError::BadSignature => TlsError::RustlsCertBadSig,
        rustls::CertificateError::Expired | rustls::CertificateError::ExpiredContext { .. } => {
            TlsError::RustlsCertExpired
        }
        rustls::CertificateError::InvalidPurpose
        | rustls::CertificateError::InvalidPurposeContext { .. } => TlsError::RustlsCertPurpose,
        rustls::CertificateError::NotValidForName
        | rustls::CertificateError::NotValidForNameContext { .. } => TlsError::RustlsCertName,
        rustls::CertificateError::NotValidYet
        | rustls::CertificateError::NotValidYetContext { .. } => TlsError::RustlsCertNotYet,
        rustls::CertificateError::UnknownIssuer => TlsError::RustlsCertUnknownIssuer,
        rustls::CertificateError::UnsupportedSignatureAlgorithmContext { .. }
        | rustls::CertificateError::UnsupportedSignatureAlgorithmForPublicKeyContext { .. } => {
            TlsError::RustlsCertSigAlg
        }
        _ => TlsError::RustlsCert,
    }
}

fn send_all(socket: TcpSocketHandle, mut buf: &[u8]) -> Result<(), TlsError> {
    for _ in 0..TLS_STEPS {
        if buf.is_empty() {
            return Ok(());
        }
        let chunk_len = core::cmp::min(buf.len(), TLS_IO_BUF);
        let n = sys_tcp_send(socket, &buf[..chunk_len]);
        if n > 0 {
            buf = &buf[n as usize..];
        } else if n == EAGAIN {
            core::hint::spin_loop();
        } else {
            return Err(TlsError::Sys(n));
        }
    }
    Err(TlsError::Limit)
}

fn recv_append(socket: TcpSocketHandle, incoming: &mut Vec<u8>) -> Result<(), TlsError> {
    let mut buf = vec![0u8; TLS_IO_BUF];
    for _ in 0..TLS_STEPS {
        let n = sys_tcp_recv(socket, &mut buf);
        if n > 0 {
            incoming.extend_from_slice(&buf[..n as usize]);
            return Ok(());
        }
        if n == EAGAIN {
            core::hint::spin_loop();
            continue;
        }
        return Err(TlsError::Sys(n));
    }
    Err(TlsError::Limit)
}

fn discard_front(buf: &mut Vec<u8>, count: usize) {
    if count == 0 {
        return;
    }
    if count >= buf.len() {
        buf.clear();
    } else {
        buf.drain(0..count);
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|item| item == needle)
}

fn content_length(headers: &[u8]) -> Result<usize, TlsError> {
    for line in headers.split(|b| *b == b'\n') {
        let line = trim_cr(line);
        if let Some(rest) = strip_prefix_ascii_case(line, b"content-length:") {
            return parse_decimal(trim_space(rest));
        }
    }
    Err(TlsError::BadHttp)
}

fn strip_prefix_ascii_case<'a>(value: &'a [u8], prefix: &[u8]) -> Option<&'a [u8]> {
    if value.len() < prefix.len() {
        return None;
    }
    for idx in 0..prefix.len() {
        if !value[idx].eq_ignore_ascii_case(&prefix[idx]) {
            return None;
        }
    }
    Some(&value[prefix.len()..])
}

fn trim_cr(value: &[u8]) -> &[u8] {
    value.strip_suffix(b"\r").unwrap_or(value)
}

fn trim_space(mut value: &[u8]) -> &[u8] {
    while matches!(value.first(), Some(b' ' | b'\t')) {
        value = &value[1..];
    }
    value
}

fn parse_decimal(value: &[u8]) -> Result<usize, TlsError> {
    let mut out = 0usize;
    for b in value {
        if !b.is_ascii_digit() {
            break;
        }
        out = out
            .checked_mul(10)
            .and_then(|n| n.checked_add((b - b'0') as usize))
            .ok_or(TlsError::BadHttp)?;
    }
    Ok(out)
}

fn push_decimal(mut value: usize, out: &mut Vec<u8>) {
    let mut buf = [0u8; 20];
    let mut idx = buf.len();
    if value == 0 {
        out.push(b'0');
        return;
    }
    while value > 0 {
        idx -= 1;
        buf[idx] = b'0' + (value % 10) as u8;
        value /= 10;
    }
    out.extend_from_slice(&buf[idx..]);
}
