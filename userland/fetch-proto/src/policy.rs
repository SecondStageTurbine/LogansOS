// SPDX-License-Identifier: MPL-2.0
//! What the broker refuses (THINK §3).
//!
//! - [`check_url`]: before dialing. `https` only; no user info; a port that is
//!   not one of the Fetch standard's bad ports; a host that is a DNS name, a
//!   strict dotted-quad IPv4 literal or a bracketed IPv6 literal, and never a
//!   private, loopback, link-local or otherwise non-public address.
//! - [`ip_allowed`]: after dialing. A public-looking name can resolve to a
//!   private address, so the broker checks the connected peer too, before it
//!   sends a byte.
//! - [`resolve_location`]: a redirect's `Location`, resolved against the
//!   current URL; the result goes through [`check_url`] again, so every hop
//!   is held to the same rules (and https → http is refused as a scheme).
//!
//! Numeric hosts are where SSRF filters usually leak: WHATWG's IPv4 parser
//! also accepts `127.1`, `0x7f.0.0.1` and `017700000001`, all loopback. So a
//! host whose last label starts with a digit must be a strict dotted quad
//! (four decimal octets, no leading zeros) or it is refused outright.

use alloc::format;
use alloc::string::{String, ToString};
use core::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// The Fetch standard's bad ports: whatwg/fetch `fetch.bs` @
/// `59afe349913d56898c947576216f1c2579308be7` (2026-10-19), the table under
/// "bad port", read from the raw source (83 entries).
pub const BAD_PORTS: [u16; 83] = [
    0, 1, 7, 9, 11, 13, 15, 17, 19, 20, 21, 22, 23, 25, 37, 42, 43, 53, 69, 77, 79, 87, 95, 101,
    102, 103, 104, 109, 110, 111, 113, 115, 117, 119, 123, 135, 137, 139, 143, 161, 179, 389, 427,
    465, 512, 513, 514, 515, 526, 530, 531, 532, 540, 548, 554, 556, 563, 587, 601, 636, 989, 990,
    993, 995, 1719, 1720, 1723, 2049, 3659, 4045, 4190, 5060, 5061, 6000, 6566, 6665, 6666, 6667,
    6668, 6669, 6679, 6697, 10080,
];

/// Longest DNS name the broker dials (RFC 1035).
pub const MAX_HOST: usize = 253;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Not `https://`.
    Scheme,
    /// `user:pass@` in the authority.
    UserInfo,
    /// Empty, too long, non-ASCII, or a character a DNS name can't have.
    Host,
    /// A numeric host that is not a strict dotted quad.
    AmbiguousNumericHost,
    /// `localhost`, `*.localhost` or `*.local`.
    LocalName,
    /// An address that is not public.
    PrivateAddress,
    /// Not a port number, or one of [`BAD_PORTS`].
    Port,
    /// A `Location` the broker can't resolve.
    Location,
}

/// A checked URL, ready to dial.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    /// Lower-case, without a trailing dot; an IPv6 literal without brackets.
    pub host: String,
    pub port: u16,
    /// The path and query, starting with `/` (the fragment is dropped).
    pub path: String,
}

impl Target {
    /// The URL back as text (the form `HEAD.final_url` carries).
    pub fn url(&self) -> String {
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        if self.port == 443 {
            format!("https://{host}{}", self.path)
        } else {
            format!("https://{host}:{}{}", self.port, self.path)
        }
    }

    /// The literal address, if the host is one.
    pub fn ip(&self) -> Option<IpAddr> {
        self.host.parse().ok()
    }
}

/// Check `url` (THINK §3) and split it into what the broker dials.
pub fn check_url(url: &str) -> Result<Target, Refusal> {
    let rest = strip_prefix_ci(url.trim(), "https://").ok_or(Refusal::Scheme)?;
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(end);
    if authority.contains('@') {
        return Err(Refusal::UserInfo);
    }
    let (host, port) = split_host_port(authority)?;
    let host = check_host(host)?;
    let tail = tail.split('#').next().unwrap_or("");
    let path = if tail.starts_with('/') {
        tail.to_string()
    } else {
        format!("/{tail}")
    };
    Ok(Target { host, port, path })
}

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let head = s.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &s[prefix.len()..])
}

fn split_host_port(authority: &str) -> Result<(&str, u16), Refusal> {
    let (host, port) = if let Some(v6) = authority.strip_prefix('[') {
        let close = v6.find(']').ok_or(Refusal::Host)?;
        let after = &v6[close + 1..];
        let port = match after {
            "" => None,
            p => Some(p.strip_prefix(':').ok_or(Refusal::Host)?),
        };
        (&authority[..close + 2], port)
    } else {
        match authority.rsplit_once(':') {
            Some((h, p)) => (h, Some(p)),
            None => (authority, None),
        }
    };
    let port = match port {
        None | Some("") => 443,
        Some(p) => {
            if p.is_empty() || p.len() > 5 || !p.bytes().all(|b| b.is_ascii_digit()) {
                return Err(Refusal::Port);
            }
            let n: u32 = p.parse().map_err(|_| Refusal::Port)?;
            u16::try_from(n).map_err(|_| Refusal::Port)?
        }
    };
    if BAD_PORTS.contains(&port) {
        return Err(Refusal::Port);
    }
    Ok((host, port))
}

fn check_host(raw: &str) -> Result<String, Refusal> {
    if let Some(inner) = raw.strip_prefix('[') {
        let inner = inner.strip_suffix(']').ok_or(Refusal::Host)?;
        let v6: Ipv6Addr = inner.parse().map_err(|_| Refusal::Host)?;
        if !ip_allowed(IpAddr::V6(v6)) {
            return Err(Refusal::PrivateAddress);
        }
        return Ok(v6.to_string());
    }
    let host = raw.strip_suffix('.').unwrap_or(raw).to_ascii_lowercase();
    if host.is_empty()
        || host.len() > MAX_HOST
        || !host
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'.')
        || host.split('.').any(|l| l.is_empty() || l.len() > 63)
    {
        return Err(Refusal::Host);
    }
    let last = host.rsplit('.').next().unwrap_or("");
    if last.as_bytes()[0].is_ascii_digit() {
        let v4 = strict_dotted_quad(&host).ok_or(Refusal::AmbiguousNumericHost)?;
        if !ip_allowed(IpAddr::V4(v4)) {
            return Err(Refusal::PrivateAddress);
        }
        return Ok(host);
    }
    if host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local") {
        return Err(Refusal::LocalName);
    }
    Ok(host)
}

/// Four decimal octets 0..=255, no leading zeros, nothing else.
fn strict_dotted_quad(host: &str) -> Option<Ipv4Addr> {
    let mut octets = [0u8; 4];
    let mut parts = host.split('.');
    for o in &mut octets {
        let p = parts.next()?;
        if p.is_empty() || p.len() > 3 || (p.len() > 1 && p.starts_with('0')) {
            return None;
        }
        if !p.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        *o = p.parse::<u16>().ok().and_then(|v| u8::try_from(v).ok())?;
    }
    parts.next().is_none().then(|| Ipv4Addr::from(octets))
}

/// Whether the broker may connect to `ip`: a globally routed unicast
/// address. The rule is the kernel's (`net_dial_cap_logic::v4_public` /
/// `v6_public`), which also enforces it in the dial for a cap without
/// `NON_PUBLIC`.
pub fn ip_allowed(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => crate::dial_logic::v4_public(v4.octets()),
        IpAddr::V6(v6) => crate::dial_logic::v6_public(v6.octets()),
    }
}

/// Resolve a redirect's `Location` against `base`. The result must still pass
/// [`check_url`]; this only builds the URL.
pub fn resolve_location(base: &Target, location: &str) -> Result<String, Refusal> {
    let loc = location.trim();
    if loc.is_empty() || loc.bytes().any(|b| b < 0x20 || b == 0x7f) {
        return Err(Refusal::Location);
    }
    if let Some(colon) = loc.find(':')
        && loc[..colon]
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'-' || b == b'.')
        && !loc[..colon].is_empty()
        && !loc.starts_with('/')
    {
        // Absolute, with a scheme: check_url decides whether it's allowed.
        return Ok(loc.to_string());
    }
    let mut origin = base.url();
    origin.truncate(origin.len() - base.path.len());
    if let Some(net) = loc.strip_prefix("//") {
        return Ok(format!("https://{net}"));
    }
    if loc.starts_with('/') {
        return Ok(format!("{origin}{loc}"));
    }
    if loc.starts_with('?') {
        let path = base.path.split('?').next().unwrap_or("/");
        return Ok(format!("{origin}{path}{loc}"));
    }
    // Relative to the base's directory.
    let path = base.path.split('?').next().unwrap_or("/");
    let dir = &path[..path.rfind('/').map(|i| i + 1).unwrap_or(1)];
    Ok(format!("{origin}{dir}{loc}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(host: &str, port: u16, path: &str) -> Target {
        Target {
            host: host.to_string(),
            port,
            path: path.to_string(),
        }
    }

    #[test]
    fn accepts_ordinary_urls() {
        assert_eq!(
            check_url("https://Example.COM/a/b?q=1#frag"),
            Ok(t("example.com", 443, "/a/b?q=1"))
        );
        assert_eq!(
            check_url("https://example.com"),
            Ok(t("example.com", 443, "/"))
        );
        assert_eq!(
            check_url("HTTPS://example.com."),
            Ok(t("example.com", 443, "/"))
        );
        assert_eq!(
            check_url("https://example.com:8443/x"),
            Ok(t("example.com", 8443, "/x"))
        );
        assert_eq!(
            check_url("https://example.com?q"),
            Ok(t("example.com", 443, "/?q"))
        );
        assert_eq!(check_url("https://8.8.8.8/"), Ok(t("8.8.8.8", 443, "/")));
        assert_eq!(
            check_url("https://[2606:4700::1111]:444/"),
            Ok(t("2606:4700::1111", 444, "/"))
        );
    }

    #[test]
    fn refuses_scheme_userinfo_ports() {
        assert_eq!(check_url("http://example.com/"), Err(Refusal::Scheme));
        assert_eq!(check_url("file:///etc/passwd"), Err(Refusal::Scheme));
        assert_eq!(check_url("javascript:alert(1)"), Err(Refusal::Scheme));
        assert_eq!(
            check_url("https://u:p@example.com/"),
            Err(Refusal::UserInfo)
        );
        assert_eq!(
            check_url("https://example.com@evil.com/"),
            Err(Refusal::UserInfo)
        );
        assert_eq!(check_url("https://example.com:25/"), Err(Refusal::Port));
        assert_eq!(check_url("https://example.com:0/"), Err(Refusal::Port));
        assert_eq!(check_url("https://example.com:65536/"), Err(Refusal::Port));
        assert_eq!(check_url("https://example.com:+443/"), Err(Refusal::Port));
        assert_eq!(check_url("https://example.com:4x3/"), Err(Refusal::Port));
    }

    #[test]
    fn bad_port_table_is_the_fetch_standards() {
        // Spot checks against the table in fetch.bs: first, last, the
        // 6665-6669 run, and two neighbours that are not in it.
        assert_eq!(BAD_PORTS.len(), 83);
        assert_eq!((BAD_PORTS[0], BAD_PORTS[82]), (0, 10080));
        for p in 6665..=6669 {
            assert!(BAD_PORTS.contains(&p));
        }
        assert!(!BAD_PORTS.contains(&443) && !BAD_PORTS.contains(&8080));
        assert!(BAD_PORTS.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn refuses_local_and_private_destinations() {
        for url in [
            "https://localhost/",
            "https://LOCALHOST./",
            "https://a.localhost/",
            "https://printer.local/",
        ] {
            assert_eq!(check_url(url), Err(Refusal::LocalName), "{url}");
        }
        for url in [
            "https://127.0.0.1/",
            "https://10.1.2.3/",
            "https://172.16.0.1/",
            "https://172.31.255.255/",
            "https://192.168.1.1/",
            "https://169.254.169.254/latest/meta-data/",
            "https://100.64.0.1/",
            "https://0.0.0.0/",
            "https://224.0.0.1/",
            "https://255.255.255.255/",
            "https://[::1]/",
            "https://[::]/",
            "https://[fd00::1]/",
            "https://[fe80::1]/",
            "https://[ff02::1]/",
            "https://[::ffff:127.0.0.1]/",
            "https://[::ffff:10.0.0.1]/",
            "https://[64:ff9b::8.8.8.8]/",
        ] {
            assert_eq!(check_url(url), Err(Refusal::PrivateAddress), "{url}");
        }
        // Next to the private ranges, but public.
        assert!(check_url("https://172.32.0.1/").is_ok());
        assert!(check_url("https://100.128.0.1/").is_ok());
        assert!(check_url("https://11.0.0.1/").is_ok());
    }

    #[test]
    fn refuses_numeric_host_tricks() {
        // All of these are loopback to WHATWG's IPv4 parser.
        for url in [
            "https://127.1/",
            "https://0x7f.0.0.1/",
            "https://0x7f000001/",
            "https://2130706433/",
            "https://017700000001/",
            "https://127.000.000.001/",
            "https://1.2.3.4.5/",
        ] {
            assert_eq!(check_url(url), Err(Refusal::AmbiguousNumericHost), "{url}");
        }
        // A digit-leading label that is not the last one is an ordinary name.
        assert!(check_url("https://1password.com/").is_ok());
        assert!(check_url("https://123.example/").is_ok());
    }

    #[test]
    fn refuses_bad_hosts() {
        for url in [
            "https:///path",
            "https://exa mple.com/",
            "https://ex%61mple.com/",
            "https://bücher.example/",
            "https://a..b/",
            "https://[::1/",
            "https://[nope]/",
        ] {
            assert_eq!(check_url(url), Err(Refusal::Host), "{url}");
        }
        let long_label = "a".repeat(64);
        assert_eq!(
            check_url(&format!("https://{long_label}.com/")),
            Err(Refusal::Host)
        );
    }

    #[test]
    fn peers_are_checked_like_literals() {
        assert!(ip_allowed("93.184.215.14".parse().unwrap()));
        assert!(ip_allowed("2606:2800:21f:cb07::1".parse().unwrap()));
        assert!(!ip_allowed("127.0.0.53".parse().unwrap()));
        assert!(!ip_allowed("::ffff:192.168.0.1".parse().unwrap()));
    }

    #[test]
    fn redirects_resolve_and_are_rechecked() {
        let base = t("example.com", 443, "/a/b/c?x=1");
        assert_eq!(
            resolve_location(&base, "https://other.org/z").unwrap(),
            "https://other.org/z"
        );
        assert_eq!(
            resolve_location(&base, "//cdn.example.com/s.js").unwrap(),
            "https://cdn.example.com/s.js"
        );
        assert_eq!(
            resolve_location(&base, "/root").unwrap(),
            "https://example.com/root"
        );
        assert_eq!(
            resolve_location(&base, "d.html").unwrap(),
            "https://example.com/a/b/d.html"
        );
        assert_eq!(
            resolve_location(&base, "?y=2").unwrap(),
            "https://example.com/a/b/c?y=2"
        );
        let on_port = t("example.com", 8443, "/p");
        assert_eq!(
            resolve_location(&on_port, "/q").unwrap(),
            "https://example.com:8443/q"
        );
        // A downgrade or a private hop resolves, and then check_url refuses it.
        let down = resolve_location(&base, "http://example.com/").unwrap();
        assert_eq!(check_url(&down), Err(Refusal::Scheme));
        let inner = resolve_location(&base, "https://10.0.0.1/admin").unwrap();
        assert_eq!(check_url(&inner), Err(Refusal::PrivateAddress));
        assert_eq!(resolve_location(&base, ""), Err(Refusal::Location));
        assert_eq!(
            resolve_location(&base, "/a\r\nSet-Cookie: x"),
            Err(Refusal::Location)
        );
    }

    #[test]
    fn target_url_round_trips() {
        for url in [
            "https://example.com/a?b",
            "https://example.com:8443/",
            "https://[2606:4700::1111]:444/x",
        ] {
            let target = check_url(url).unwrap();
            assert_eq!(check_url(&target.url()).unwrap(), target, "{url}");
        }
    }
}
