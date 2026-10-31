// SPDX-License-Identifier: MPL-2.0
//! Pure NetDialCap scope rules, kept kernel-free for the host KAT.

/// The owner/root capability may derive any concrete hostname.
pub const ROOT_HOST_PATTERN: &str = "*";
/// Port zero is reserved for the owner/root wildcard; dial targets must be non-zero.
pub const ROOT_PORT: u16 = 0;

/// Whether `scope_host:scope_port` authorises this concrete dial target.
///
/// Concrete hostnames compare ASCII-case-insensitively because DNS names do.
/// The only pattern syntax is the root `*`; durable user grants remain the exact
/// `Grant::NetDial { host, port }` shape.
pub fn scope_allows(scope_host: &str, scope_port: u16, host: &str, port: u16) -> bool {
    let host_allowed = scope_host == ROOT_HOST_PATTERN || scope_host.eq_ignore_ascii_case(host);
    let port_allowed = scope_port == ROOT_PORT || scope_port == port;
    host_allowed && port_allowed
}

/// Scope attenuation is monotonic: a child must be concrete and inside its parent.
pub fn can_attenuate(
    parent_host: &str,
    parent_port: u16,
    child_host: &str,
    child_port: u16,
) -> bool {
    !child_host.is_empty()
        && child_host != ROOT_HOST_PATTERN
        && child_port != ROOT_PORT
        && scope_allows(parent_host, parent_port, child_host, child_port)
}

/// Kill-switch truth table: disabled enforcement is the ambient pre-feature path.
///
/// The paired runtime expression is the `net-dial-cap` cfg split in
/// `arch/x86_64/syscall.rs`; edits to either expression must preserve this table.
pub const fn authority_allows(enforcement_enabled: bool, matching_cap_held: bool) -> bool {
    !enforcement_enabled || matching_cap_held
}

/// OT-61 rung 5b: whether an IPv4 address is public (globally routed
/// unicast). A dial cap without `net_dial_rights::NON_PUBLIC` reaches only
/// these. Refused: 0/8, 10/8, 100.64/10 (CGNAT), 127/8, 169.254/16, 172.16/12,
/// 192.0.0/24, 192.0.2/24, 192.168/16, 198.18/15, 198.51.100/24,
/// 203.0.113/24, and 224/4 upwards (multicast, reserved, broadcast).
///
/// The broker's pre-dial check (`userland/fetch-proto`, `policy::ip_allowed`)
/// includes this file, so the two can't disagree.
pub const fn v4_public(o: [u8; 4]) -> bool {
    let [a, b, c, _] = o;
    let refused = a == 0
        || a == 10
        || (a == 100 && b >= 64 && b < 128)
        || a == 127
        || (a == 169 && b == 254)
        || (a == 172 && b >= 16 && b < 32)
        || (a == 192 && b == 0 && (c == 0 || c == 2))
        || (a == 192 && b == 168)
        || (a == 198 && (b == 18 || b == 19))
        || (a == 198 && b == 51 && c == 100)
        || (a == 203 && b == 0 && c == 113)
        || a >= 224;
    !refused
}

/// The IPv6 rule. Refused: `::` and `::1`; addresses embedding IPv4
/// (`::ffff:0:0/96` is judged by its IPv4 address; `::/96` and the NAT64
/// prefix `64:ff9b::/96` are refused outright); `fc00::/7`, `fe80::/10`,
/// `ff00::/8`, and the documentation prefix `2001:db8::/32`.
pub const fn v6_public(a: [u8; 16]) -> bool {
    let mut zero_prefix = true;
    let mut i = 0;
    while i < 10 {
        if a[i] != 0 {
            zero_prefix = false;
        }
        i += 1;
    }
    if zero_prefix && a[10] == 0xff && a[11] == 0xff {
        return v4_public([a[12], a[13], a[14], a[15]]);
    }
    if zero_prefix && a[10] == 0 && a[11] == 0 {
        // ::, ::1 and the deprecated IPv4-compatible ::/96.
        return false;
    }
    let nat64 = a[0] == 0x00
        && a[1] == 0x64
        && a[2] == 0xff
        && a[3] == 0x9b
        && a[4] == 0
        && a[5] == 0
        && a[6] == 0
        && a[7] == 0
        && a[8] == 0
        && a[9] == 0
        && a[10] == 0
        && a[11] == 0;
    let unique_local = a[0] & 0xfe == 0xfc;
    let link_local = a[0] == 0xfe && a[1] & 0xc0 == 0x80;
    let multicast = a[0] == 0xff;
    let documentation = a[0] == 0x20 && a[1] == 0x01 && a[2] == 0x0d && a[3] == 0xb8;
    !(nat64 || unique_local || link_local || multicast || documentation)
}

/// Whether a dial may use a resolved candidate: always with the
/// `NON_PUBLIC` right, otherwise only a public address.
pub const fn candidate_allowed(non_public_right: bool, public: bool) -> bool {
    non_public_right || public
}
