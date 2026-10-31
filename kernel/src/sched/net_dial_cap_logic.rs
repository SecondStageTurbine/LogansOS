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
