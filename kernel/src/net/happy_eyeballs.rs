// SPDX-License-Identifier: MPL-2.0
//! RFC 8305-style name dial helper for Phase 4 BUILD-5b.

extern crate alloc;

use alloc::sync::Arc;

use crate::net::dns::{self, FamilyHint, NetCandidate};
use crate::net::socket::{AF_INET6, SocketAddr};
use crate::net::tcp::{self, FourTuple, TcpSocket, TcpState};
use crate::sched::process::ProcessId;

pub const DEFAULT_RESOLUTION_DELAY_TICKS: u32 = 1;
pub const DEFAULT_CONNECTION_ATTEMPT_DELAY_TICKS: u32 = 5;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum DialError {
    Invalid,
    Resolve,
    NoCandidates,
    AllFailed,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct DialDelays {
    pub resolution_delay_ticks: u32,
    pub connection_attempt_delay_ticks: u32,
}

impl DialDelays {
    pub const fn default() -> Self {
        Self {
            resolution_delay_ticks: DEFAULT_RESOLUTION_DELAY_TICKS,
            connection_attempt_delay_ticks: DEFAULT_CONNECTION_ATTEMPT_DELAY_TICKS,
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct RaceOutcome {
    pub winner: NetCandidate,
    pub attempts_started: usize,
    pub cancelled_losers: usize,
}

pub fn sort_candidates_prefer_ipv6(candidates: &mut [NetCandidate]) {
    candidates.sort_by_key(|candidate| {
        if candidate.family == AF_INET6 as u16 {
            0u8
        } else {
            1u8
        }
    });
}

pub fn race_candidates_with<F>(
    candidates: &[NetCandidate],
    _delays: DialDelays,
    mut attempt: F,
) -> Result<RaceOutcome, DialError>
where
    F: FnMut(NetCandidate) -> bool,
{
    if candidates.is_empty() {
        return Err(DialError::NoCandidates);
    }
    let mut ordered = candidates.to_vec();
    sort_candidates_prefer_ipv6(&mut ordered);
    let mut attempts_started = 0usize;
    for candidate in ordered {
        attempts_started = attempts_started.saturating_add(1);
        if attempt(candidate) {
            return Ok(RaceOutcome {
                winner: candidate,
                attempts_started,
                cancelled_losers: candidates.len().saturating_sub(attempts_started),
            });
        }
    }
    Err(DialError::AllFailed)
}

pub fn dial_name(owner: ProcessId, name: &str, port: u16) -> Result<Arc<TcpSocket>, DialError> {
    if port == 0 {
        return Err(DialError::Invalid);
    }
    let candidates = dns::resolve(name, FamilyHint::Any).map_err(|_| DialError::Resolve)?;
    dial_candidates(owner, &candidates, port, DialDelays::default())
}

pub fn dial_candidates(
    owner: ProcessId,
    candidates: &[NetCandidate],
    port: u16,
    delays: DialDelays,
) -> Result<Arc<TcpSocket>, DialError> {
    let mut winner = None;
    let outcome = race_candidates_with(candidates, delays, |candidate| {
        let Ok(socket) = start_tcp_attempt(owner, candidate, port) else {
            return false;
        };
        winner = Some(socket);
        true
    })?;
    if outcome.attempts_started == 0 {
        return Err(DialError::AllFailed);
    }
    winner.ok_or(DialError::AllFailed)
}

fn start_tcp_attempt(
    owner: ProcessId,
    candidate: NetCandidate,
    port: u16,
) -> Result<Arc<TcpSocket>, DialError> {
    let remote = candidate
        .socket_addr(port)
        .map_err(|_| DialError::Invalid)?;
    let selected_port = tcp::allocate_ephemeral_port();
    let local =
        tcp::select_local_source(remote, selected_port).map_err(|_| DialError::AllFailed)?;
    insert_active_socket(owner, local, remote)
}

fn insert_active_socket(
    owner: ProcessId,
    local: SocketAddr,
    remote: SocketAddr,
) -> Result<Arc<TcpSocket>, DialError> {
    let tuple = FourTuple::new(local, remote).map_err(|_| DialError::Invalid)?;
    let socket = Arc::new(TcpSocket::new(owner, tuple, TcpState::SynSent));
    if tcp::insert_connection(socket.clone()).is_err() {
        return Err(DialError::AllFailed);
    }
    if socket.start_active_open().is_err() {
        tcp::remove_connection(tuple);
        return Err(DialError::AllFailed);
    }
    Ok(socket)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::socket::{AF_INET, AF_INET6};

    fn candidate(family: u32, last: u8) -> NetCandidate {
        let mut addr = [0u8; 16];
        if family == AF_INET {
            addr[..4].copy_from_slice(&[192, 0, 2, last]);
        } else {
            addr = [0x20, 1, 0xd, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, last];
        }
        NetCandidate {
            family: family as u16,
            reserved: 0,
            ttl: 30,
            addr,
        }
    }

    #[test]
    fn rfc6724_default_prefers_ipv6() {
        let mut candidates = [candidate(AF_INET, 1), candidate(AF_INET6, 2)];
        sort_candidates_prefer_ipv6(&mut candidates);
        assert_eq!(candidates[0].family, AF_INET6 as u16);
    }

    #[test]
    fn race_falls_back_to_ipv4_when_ipv6_fails() {
        let candidates = [candidate(AF_INET, 1), candidate(AF_INET6, 2)];
        let outcome = race_candidates_with(&candidates, DialDelays::default(), |candidate| {
            candidate.family != AF_INET6 as u16
        });
        assert!(matches!(
            outcome,
            Ok(RaceOutcome {
                winner,
                attempts_started: 2,
                ..
            }) if winner.family == AF_INET as u16
        ));
    }

    #[test]
    fn empty_and_all_fail_are_clean_errors() {
        assert_eq!(
            race_candidates_with(&[], DialDelays::default(), |_candidate| true),
            Err(DialError::NoCandidates)
        );
        let candidates = [candidate(AF_INET6, 2)];
        assert_eq!(
            race_candidates_with(&candidates, DialDelays::default(), |_candidate| false),
            Err(DialError::AllFailed)
        );
    }
}
