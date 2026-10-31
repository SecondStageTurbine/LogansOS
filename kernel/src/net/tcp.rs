// SPDX-License-Identifier: MPL-2.0
//! Hand-rolled TCP foundation for Phase 4 BUILD-4.

extern crate alloc;

use alloc::collections::{BTreeMap, VecDeque};
use alloc::sync::Arc;
use alloc::{vec, vec::Vec};
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use spin::{Lazy, Mutex};

use crate::drivers::net;
use crate::net::arp;
use crate::net::interface;
use crate::net::ipv4;
use crate::net::ndp;
use crate::net::socket::{AddressFamily, SocketAddr};
use crate::net::tls_record::{self, TlsSocketState};
use crate::sched::process::ProcessId;

pub const IPV4_PROTOCOL_TCP: u8 = 6;
pub const TCP_HEADER_LEN: usize = 20;
pub const TCP_MAX_HEADER_LEN: usize = 60;
pub const TCP_MAX_OPTION_LEN: usize = TCP_MAX_HEADER_LEN - TCP_HEADER_LEN;

static CONNECTIONS: Lazy<Mutex<BTreeMap<FourTuple, Arc<TcpSocket>>>> =
    Lazy::new(|| Mutex::new(BTreeMap::new()));
static LISTENERS: Lazy<Mutex<BTreeMap<ListenKey, Arc<TcpListener>>>> =
    Lazy::new(|| Mutex::new(BTreeMap::new()));
/// Chad #189, #241 1c: with `net-tick`, `enqueue` admits a segment only
/// while the queue holds fewer than `OUTBOUND_ADMIT` (past it, the segment is
/// dropped and counted). It is an admission threshold, not a hard bound on
/// the queue: segments a pass took out and keeps (wire ones the loopback
/// pump skipped), and those the syscalls' pump retains, go back whatever the
/// length then, so producers on other CPUs can push it past the threshold by
/// what is out at that moment (512 + 16 seen on the host). That overshoot is
/// itself bounded: only what was admitted before comes back. The queue is
/// made with room for the threshold plus a pump's budget, so a put-back
/// rarely reallocates it.
#[cfg(feature = "net-tick")]
pub const OUTBOUND_ADMIT: usize = 512;
#[cfg(feature = "net-tick")]
static OUTBOUND_DROPS: AtomicU64 = AtomicU64::new(0);

/// Segments dropped because `OUTBOUND` was full (`net-tick`).
#[cfg(feature = "net-tick")]
pub fn outbound_drops() -> u64 {
    OUTBOUND_DROPS.load(Ordering::Relaxed)
}

#[cfg(not(feature = "net-tick"))]
static OUTBOUND: Lazy<Mutex<VecDeque<TcpOutboundSegment>>> =
    Lazy::new(|| Mutex::new(VecDeque::new()));
#[cfg(feature = "net-tick")]
static OUTBOUND: Lazy<Mutex<VecDeque<TcpOutboundSegment>>> =
    Lazy::new(|| Mutex::new(VecDeque::with_capacity(OUTBOUND_ADMIT + PUMP_BUDGET)));
static TIME_WAITS: Lazy<Mutex<BTreeMap<FourTuple, TimeWaitEntry>>> =
    Lazy::new(|| Mutex::new(BTreeMap::new()));
static LAST_WIRE_EGRESS: Lazy<Mutex<Option<TcpWireEgressCapture>>> = Lazy::new(|| Mutex::new(None));
static NEXT_EPHEMERAL_PORT: AtomicU32 = AtomicU32::new(49152);
static NEXT_ISS: AtomicU32 = AtomicU32::new(0x6e65_6f00);
static CLOCK_TICKS: AtomicU32 = AtomicU32::new(0);
/// OT-94: SYNs refused because the listener's half-open or accept queue was
/// full (each answered with a RST, nothing allocated).
static SYNS_REFUSED: AtomicU64 = AtomicU64::new(0);

/// #202 F1 / #170 discriminator (`tcp_listen_bounds`): the scenario parks a
/// listener here with the point where a second CPU's close should land, and
/// the receive path runs that close there, deterministically.
///
/// IRQ-safe (#230.3, Chad #183, CP-37): the harness parks and reaches it with
/// IF on, and with `net-tick` in the build the periodic thread's IF-off pump
/// reaches `race_point` too, so a plain spin lock held by a preempted IF-on
/// taker could stall that pump on the same CPU.
#[cfg(feature = "neo-test-tcp-listen-bounds")]
static RACE_CLOSE: crate::sync::irqsafe_mutex::IrqSafeMutex<Option<(RaceAt, Arc<TcpListener>)>> =
    crate::sync::irqsafe_mutex::IrqSafeMutex::new(None);

/// Where [`RACE_CLOSE`]'s close runs.
#[cfg(feature = "neo-test-tcp-listen-bounds")]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RaceAt {
    /// A SYN found the listener, before it reserves a half-open place.
    AfterLookup,
    /// The SYN reserved its place, before its connection enters `CONNECTIONS`.
    BeforeInsert,
    /// An ACK took its connection out of the half-open queue, before it
    /// enters the accept queue.
    BeforeEnqueue,
}

/// Test-only: close `listener` at `at` in the next receive that gets there.
#[cfg(feature = "neo-test-tcp-listen-bounds")]
pub fn race_close_at(at: RaceAt, listener: Arc<TcpListener>) {
    *RACE_CLOSE.lock() = Some((at, listener));
}

/// Test-only: whether a close parked with [`race_close_at`] is still waiting.
#[cfg(feature = "neo-test-tcp-listen-bounds")]
pub fn race_close_pending() -> bool {
    RACE_CLOSE.lock().is_some()
}

#[cfg(feature = "neo-test-tcp-listen-bounds")]
fn race_point(at: RaceAt) {
    let parked = {
        let mut slot = RACE_CLOSE.lock();
        match slot.as_ref() {
            Some((want, _)) if *want == at => slot.take(),
            _ => None,
        }
    };
    if let Some((_, listener)) = parked {
        close_listener(&listener);
    }
}

const DEFAULT_WINDOW: u16 = 65535;
pub(crate) const RX_QUEUE_LIMIT: usize = 64 * 1024;
const RTO_TICKS: u32 = 3;
const TIME_WAIT_TICKS: u32 = 10;
/// OT-94: a half-open connection (SYN received, handshake not finished) is
/// reaped this long after its SYN, in timer ticks (100 Hz): 3 s. The timer
/// tick, not the TCP clock, because nothing advances the TCP clock on a
/// product kernel once bring-up is done.
const SYN_RECEIVED_TIMEOUT_TICKS: u64 = 300;
const TCP_OPTION_END: u8 = 0;
const TCP_OPTION_NOP: u8 = 1;
const TCP_OPTION_WINDOW_SCALE: u8 = 3;
const TCP_WINDOW_SCALE_SHIFT: u8 = 2;
const TCP_MSS: u32 = 1460;
const CUBIC_INITIAL_WINDOW: u32 = TCP_MSS * 10;
const CUBIC_BETA_NUM: u32 = 717;
const CUBIC_BETA_DEN: u32 = 1024;
const CUBIC_SCALE_NUM: i64 = 410;
const CUBIC_SCALE_DEN: i64 = 1024;
const ETHERNET_HEADER_LEN: usize = 14;
const ETHERNET_IPV6: u16 = 0x86dd;
const IPV6_HEADER_LEN: usize = 40;
const DEFAULT_HOP_LIMIT: u8 = 64;
const TCP_WINDOW_SCALE_OPTION: [u8; 4] = [
    TCP_OPTION_WINDOW_SCALE,
    3,
    TCP_WINDOW_SCALE_SHIFT,
    TCP_OPTION_NOP,
];

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum TcpError {
    Invalid,
    Busy,
    WouldBlock,
    BufferTooSmall,
    Checksum,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum IpAddress {
    V4([u8; 4]),
    V6([u8; 16]),
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Ord, PartialOrd)]
pub struct FourTuple {
    pub local: SocketAddr,
    pub remote: SocketAddr,
}

impl FourTuple {
    pub fn new(local: SocketAddr, remote: SocketAddr) -> Result<Self, TcpError> {
        if local.family != remote.family || local.port == 0 || remote.port == 0 {
            return Err(TcpError::Invalid);
        }
        Ok(Self { local, remote })
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum TcpState {
    Closed,
    Listen,
    SynSent,
    SynReceived,
    Established,
    FinWait1,
    FinWait2,
    CloseWait,
    Closing,
    LastAck,
    TimeWait,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum TcpEvent {
    PassiveOpen,
    ActiveOpen,
    ReceiveSyn,
    ReceiveSynAck,
    ReceiveAck,
    ReceiveFin,
    Close,
    Timeout2Msl,
    ReceiveRst,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct TcpFlags(u16);

impl TcpFlags {
    pub const FIN: Self = Self(1 << 0);
    pub const SYN: Self = Self(1 << 1);
    pub const RST: Self = Self(1 << 2);
    pub const PSH: Self = Self(1 << 3);
    pub const ACK: Self = Self(1 << 4);
    pub const URG: Self = Self(1 << 5);
    pub const ECE: Self = Self(1 << 6);
    pub const CWR: Self = Self(1 << 7);
    pub const NS: Self = Self(1 << 8);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn bits(self) -> u16 {
        self.0
    }

    pub const fn contains(self, flag: Self) -> bool {
        (self.0 & flag.0) == flag.0
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct TcpHeader {
    pub src_port: u16,
    pub dst_port: u16,
    pub sequence: u32,
    pub acknowledgment: u32,
    pub flags: TcpFlags,
    pub window: u16,
    pub urgent_pointer: u16,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct TcpSegment<'a> {
    pub header: TcpHeader,
    pub options: &'a [u8],
    pub payload: &'a [u8],
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct TcpBuildSegment<'a> {
    pub header: TcpHeader,
    pub options: &'a [u8],
    pub payload: &'a [u8],
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct TcpOutboundSegment {
    pub src: SocketAddr,
    pub dst: SocketAddr,
    pub flags: TcpFlags,
    pub sequence: u32,
    pub acknowledgment: u32,
    pub segment: Vec<u8>,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct TcpWireEgressCapture {
    pub src: SocketAddr,
    pub dst: SocketAddr,
    pub frame_len: usize,
    pub ip_version: u8,
    pub protocol: u8,
}

pub struct TcpSocket {
    owner: ProcessId,
    tuple: FourTuple,
    inner: Mutex<TcpSocketInner>,
    pub handles: crate::net::HandleCount,
}

struct TcpSocketInner {
    state: TcpState,
    iss: u32,
    irs: u32,
    snd_una: u32,
    snd_nxt: u32,
    rcv_nxt: u32,
    rx_queue: VecDeque<u8>,
    unacked: VecDeque<UnackedSegment>,
    send_window_scale: u8,
    recv_window_scale: u8,
    peer_window_scale: u8,
    cubic: CubicState,
    challenge_ack_after: u32,
    challenge_acks: u64,
    tls: Option<TlsSocketState>,
}

struct UnackedSegment {
    sequence: u32,
    acknowledgment: u32,
    flags: TcpFlags,
    payload: Vec<u8>,
    end_sequence: u32,
    retransmit_at: u32,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
struct TimeWaitEntry {
    owner: ProcessId,
    expires_at: u32,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct CubicSnapshot {
    pub cwnd: u32,
    pub ssthresh: u32,
    pub w_max: u32,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
struct CubicState {
    cwnd: u32,
    ssthresh: u32,
    w_max: u32,
    epoch_start: Option<u32>,
    origin_point: u32,
    k_ticks: u32,
}

pub struct TcpListener {
    owner: ProcessId,
    local: SocketAddr,
    backlog: usize,
    accept_queue: Mutex<VecDeque<Arc<TcpSocket>>>,
    /// OT-94: the half-open connections this listener created, oldest first,
    /// at most `backlog`. Whoever takes a socket out of this queue owns what
    /// happens to it next (accept, or close).
    half_open: Mutex<VecDeque<HalfOpen>>,
    /// #202 F1 / #170: set by `close_listener` before it drains the queues.
    /// Both queues admit only while it is clear, checked under their own
    /// lock, so a receive racing the close either lands before the drain
    /// (which takes it) or is refused; none is left with no owner.
    retired: AtomicBool,
    pub handles: crate::net::HandleCount,
}

struct HalfOpen {
    socket: Arc<TcpSocket>,
    deadline: u64,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug, Ord, PartialOrd)]
struct ListenKey {
    family: AddressFamily,
    port: u16,
    addr: [u8; 16],
}

impl TcpSocket {
    pub fn new(owner: ProcessId, tuple: FourTuple, state: TcpState) -> Self {
        Self {
            owner,
            tuple,
            inner: Mutex::new(TcpSocketInner {
                state,
                iss: 0,
                irs: 0,
                snd_una: 0,
                snd_nxt: 0,
                rcv_nxt: 0,
                rx_queue: VecDeque::new(),
                unacked: VecDeque::new(),
                send_window_scale: TCP_WINDOW_SCALE_SHIFT,
                recv_window_scale: TCP_WINDOW_SCALE_SHIFT,
                peer_window_scale: 0,
                cubic: CubicState::new(),
                challenge_ack_after: 0,
                challenge_acks: 0,
                tls: None,
            }),
            handles: crate::net::HandleCount::default(),
        }
    }

    pub fn owner(&self) -> ProcessId {
        self.owner
    }

    pub fn tuple(&self) -> FourTuple {
        self.tuple
    }

    pub fn state(&self) -> TcpState {
        self.inner.lock().state
    }

    pub fn apply_event(&self, event: TcpEvent) -> Result<TcpState, TcpError> {
        let mut inner = self.inner.lock();
        let next = transition(inner.state, event).ok_or(TcpError::Invalid)?;
        inner.state = next;
        Ok(next)
    }

    pub fn close(&self) {
        self.inner.lock().state = TcpState::Closed;
    }

    pub fn start_active_open(&self) -> Result<(), TcpError> {
        let (seq, ack) = {
            let mut inner = self.inner.lock();
            if inner.state != TcpState::SynSent {
                return Err(TcpError::Invalid);
            }
            let iss = next_iss();
            inner.iss = iss;
            inner.snd_una = iss;
            inner.snd_nxt = iss.wrapping_add(1);
            (iss, 0)
        };
        enqueue_tcp_segment_with_options(
            self.tuple,
            TcpFlags::SYN,
            seq,
            ack,
            &TCP_WINDOW_SCALE_OPTION,
            &[],
        )
    }

    pub fn send_stream(&self, data: &[u8]) -> Result<usize, TcpError> {
        if data.is_empty() {
            return Err(TcpError::Invalid);
        }
        let (seq, ack, wire_payload) = {
            let mut inner = self.inner.lock();
            if inner.state != TcpState::Established {
                return Err(TcpError::WouldBlock);
            }
            let wire_len = if inner.tls.is_some() {
                tls_record::sealed_len(data.len()).map_err(|_| TcpError::Invalid)?
            } else {
                data.len()
            };
            if bytes_in_flight(&inner).saturating_add(wire_len as u32) > inner.cubic.cwnd {
                return Err(TcpError::WouldBlock);
            }
            let wire_payload = if let Some(tls) = inner.tls.as_mut() {
                match tls.seal_application_data(data) {
                    Ok(record) => record,
                    Err(_) => {
                        inner.state = TcpState::Closed;
                        return Err(TcpError::Invalid);
                    }
                }
            } else {
                data.to_vec()
            };
            let seq = inner.snd_nxt;
            let ack = inner.rcv_nxt;
            let end_sequence = inner.snd_nxt.wrapping_add(wire_payload.len() as u32);
            inner.snd_nxt = end_sequence;
            inner.unacked.push_back(UnackedSegment {
                sequence: seq,
                acknowledgment: ack,
                flags: TcpFlags::PSH.union(TcpFlags::ACK),
                payload: wire_payload.clone(),
                end_sequence,
                retransmit_at: now().wrapping_add(RTO_TICKS),
            });
            (seq, ack, wire_payload)
        };
        enqueue_tcp_segment(
            self.tuple,
            TcpFlags::PSH.union(TcpFlags::ACK),
            seq,
            ack,
            &wire_payload,
        )?;
        Ok(data.len())
    }

    pub fn enable_tls(&self, key_material: &mut [u8]) -> Result<(), TcpError> {
        let tls = TlsSocketState::from_key_material(key_material).map_err(|_| TcpError::Invalid)?;
        let mut inner = self.inner.lock();
        if inner.state != TcpState::Established || inner.tls.is_some() {
            return Err(TcpError::Invalid);
        }
        inner.tls = Some(tls);
        Ok(())
    }

    pub fn recv_stream(&self, out: &mut [u8]) -> Result<usize, TcpError> {
        if out.is_empty() {
            return Err(TcpError::Invalid);
        }
        let mut inner = self.inner.lock();
        if inner.rx_queue.is_empty() {
            return Err(TcpError::WouldBlock);
        }
        let n = core::cmp::min(out.len(), inner.rx_queue.len());
        for slot in &mut out[..n] {
            *slot = inner.rx_queue.pop_front().ok_or(TcpError::Invalid)?;
        }
        Ok(n)
    }

    pub fn cubic_snapshot(&self) -> CubicSnapshot {
        self.inner.lock().cubic.snapshot()
    }

    pub fn challenge_ack_count(&self) -> u64 {
        self.inner.lock().challenge_acks
    }

    pub fn shutdown_write(&self) -> Result<(), TcpError> {
        let (flags, seq, ack) = {
            let mut inner = self.inner.lock();
            match inner.state {
                TcpState::Established => inner.state = TcpState::FinWait1,
                TcpState::CloseWait => inner.state = TcpState::LastAck,
                TcpState::FinWait1 | TcpState::FinWait2 | TcpState::LastAck => {
                    return Ok(());
                }
                _ => return Err(TcpError::Invalid),
            }
            let seq = inner.snd_nxt;
            let ack = inner.rcv_nxt;
            let end_sequence = inner.snd_nxt.wrapping_add(1);
            inner.snd_nxt = end_sequence;
            inner.unacked.push_back(UnackedSegment {
                sequence: seq,
                acknowledgment: ack,
                flags: TcpFlags::FIN.union(TcpFlags::ACK),
                payload: Vec::new(),
                end_sequence,
                retransmit_at: now().wrapping_add(RTO_TICKS),
            });
            (TcpFlags::FIN.union(TcpFlags::ACK), seq, ack)
        };
        enqueue_tcp_segment(self.tuple, flags, seq, ack, &[])
    }
}

impl TcpListener {
    pub fn new(owner: ProcessId, local: SocketAddr, backlog: usize) -> Result<Self, TcpError> {
        if local.port == 0 || backlog == 0 {
            return Err(TcpError::Invalid);
        }
        Ok(Self {
            owner,
            local,
            backlog,
            accept_queue: Mutex::new(VecDeque::new()),
            half_open: Mutex::new(VecDeque::new()),
            retired: AtomicBool::new(false),
            handles: crate::net::HandleCount::default(),
        })
    }

    pub fn owner(&self) -> ProcessId {
        self.owner
    }

    pub fn local(&self) -> SocketAddr {
        self.local
    }

    pub fn backlog(&self) -> usize {
        self.backlog
    }

    pub fn enqueue_accept(&self, socket: Arc<TcpSocket>) -> Result<(), TcpError> {
        let mut queue = self.accept_queue.lock();
        if self.is_retired() {
            return Err(TcpError::Invalid);
        }
        if queue.len() >= self.backlog {
            return Err(TcpError::WouldBlock);
        }
        queue.push_back(socket);
        Ok(())
    }

    pub fn dequeue_accept(&self) -> Result<Arc<TcpSocket>, TcpError> {
        self.accept_queue
            .lock()
            .pop_front()
            .ok_or(TcpError::WouldBlock)
    }

    /// Whether `close_listener` has begun: no queue admits any more.
    pub fn is_retired(&self) -> bool {
        self.retired.load(Ordering::SeqCst)
    }

    fn accept_full(&self) -> bool {
        self.accept_queue.lock().len() >= self.backlog
    }

    /// The half-open connections this listener holds.
    pub fn half_open_count(&self) -> usize {
        self.half_open.lock().len()
    }

    /// Record `socket` as half-open, unless `backlog` already are or the
    /// listener is retired.
    fn reserve_half_open(&self, socket: &Arc<TcpSocket>, now: u64) -> bool {
        let mut queue = self.half_open.lock();
        if self.is_retired() || queue.len() >= self.backlog {
            return false;
        }
        queue.push_back(HalfOpen {
            socket: socket.clone(),
            deadline: now.saturating_add(SYN_RECEIVED_TIMEOUT_TICKS),
        });
        true
    }

    /// Take `socket` out of the half-open queue; `None` if it is not there
    /// (already reaped or taken by someone else).
    fn take_half_open(&self, socket: *const TcpSocket) -> Option<Arc<TcpSocket>> {
        let mut queue = self.half_open.lock();
        let index = queue
            .iter()
            .position(|entry| core::ptr::eq(Arc::as_ptr(&entry.socket), socket))?;
        queue.remove(index).map(|entry| entry.socket)
    }

    /// Take every half-open connection whose deadline has passed. The
    /// timeout is constant, so the queue is in deadline order.
    fn take_expired_half_open(&self, now: u64) -> Vec<Arc<TcpSocket>> {
        let mut queue = self.half_open.lock();
        let mut expired = Vec::new();
        while matches!(queue.front(), Some(entry) if entry.deadline <= now) {
            if let Some(entry) = queue.pop_front() {
                expired.push(entry.socket);
            }
        }
        expired
    }

    fn take_all_half_open(&self) -> Vec<Arc<TcpSocket>> {
        self.half_open
            .lock()
            .drain(..)
            .map(|entry| entry.socket)
            .collect()
    }
}

/// OT-94: close the expired half-open connections of `listener`, each with a
/// RST to its peer.
fn reap_half_open(listener: &TcpListener, now: u64) {
    for socket in listener.take_expired_half_open(now) {
        reset_and_close(&socket);
    }
}

/// Send `socket`'s peer a RST and close it. For connections no process has a
/// handle to (half-open, or refused at accept).
fn reset_and_close(socket: &Arc<TcpSocket>) {
    let (seq, ack) = {
        let inner = socket.inner.lock();
        (inner.snd_nxt, inner.rcv_nxt)
    };
    let _ = enqueue_tcp_segment(socket.tuple, TcpFlags::RST, seq, ack, &[]);
    close_connection(socket);
}

fn timer_now() -> u64 {
    crate::arch::x86_64::timer::TICKS.load(Ordering::Relaxed)
}

/// OT-94: SYNs refused so far (test-only reader).
#[cfg(feature = "neo-test")]
pub fn syns_refused() -> u64 {
    SYNS_REFUSED.load(Ordering::Relaxed)
}

pub fn insert_listener(listener: Arc<TcpListener>) -> Result<(), TcpError> {
    let key = listen_key(listener.local);
    let mut table = LISTENERS.lock();
    if table.contains_key(&key) {
        return Err(TcpError::Busy);
    }
    table.insert(key, listener);
    Ok(())
}

pub fn remove_listener(local: SocketAddr) -> Option<Arc<TcpListener>> {
    LISTENERS.lock().remove(&listen_key(local))
}

pub fn listener_count() -> usize {
    LISTENERS.lock().len()
}

pub fn insert_connection(socket: Arc<TcpSocket>) -> Result<(), TcpError> {
    let tuple = socket.tuple();
    let mut table = CONNECTIONS.lock();
    if table.contains_key(&tuple) {
        return Err(TcpError::Busy);
    }
    table.insert(tuple, socket);
    Ok(())
}

pub fn lookup_connection(tuple: FourTuple) -> Option<Arc<TcpSocket>> {
    CONNECTIONS.lock().get(&tuple).cloned()
}

pub fn remove_connection(tuple: FourTuple) -> Option<Arc<TcpSocket>> {
    CONNECTIONS.lock().remove(&tuple)
}

/// Close `socket` and take it out of `CONNECTIONS`, but only if the entry
/// for its four-tuple is still this socket: a later connection may reuse the
/// tuple, and closing an old handle must not remove that one.
pub fn close_connection(socket: &Arc<TcpSocket>) {
    socket.close();
    let removed = {
        let mut table = CONNECTIONS.lock();
        match table.get(&socket.tuple) {
            Some(entry) if Arc::ptr_eq(entry, socket) => table.remove(&socket.tuple),
            _ => None,
        }
    };
    drop(removed);
}

/// Close `listener`: take it out of `LISTENERS` (only if the entry is still
/// this listener), then drop the connections it holds that no process has a
/// handle to yet: the ones waiting in its accept queue and its half-open ones.
/// Without this they would stay in `CONNECTIONS` for good.
///
/// #202 F1 / #170: it retires the listener first. A SYN or ACK that found it
/// before then and admits after is refused by the queue (`retired`, checked
/// under the queue's lock); one that admitted before is in a queue the drain
/// below takes. `passive_open` re-checks after it inserts its connection, for
/// a drain that ran between its reservation and the insert.
pub fn close_listener(listener: &Arc<TcpListener>) {
    listener.retired.store(true, Ordering::SeqCst);
    let key = listen_key(listener.local);
    let removed = {
        let mut table = LISTENERS.lock();
        match table.get(&key) {
            Some(entry) if Arc::ptr_eq(entry, listener) => table.remove(&key),
            _ => None,
        }
    };
    drop(removed);
    while let Ok(queued) = listener.dequeue_accept() {
        close_connection(&queued);
    }
    for socket in listener.take_all_half_open() {
        reset_and_close(&socket);
    }
}

pub fn connection_count() -> usize {
    CONNECTIONS.lock().len()
}

pub fn time_wait_count() -> usize {
    TIME_WAITS.lock().len()
}

pub fn inject_segment(src: SocketAddr, dst: SocketAddr, packet: &[u8]) -> Result<(), TcpError> {
    if src.family != dst.family || src.port == 0 || dst.port == 0 {
        return Err(TcpError::Invalid);
    }
    let segment = parse_segment(ip_from_socket_addr(src), ip_from_socket_addr(dst), packet)?;
    let tuple = FourTuple::new(dst, src)?;
    if let Some(socket) = lookup_connection(tuple) {
        return socket.receive_segment(segment);
    }
    if segment.header.flags.contains(TcpFlags::SYN) && !segment.header.flags.contains(TcpFlags::ACK)
    {
        return passive_open(src, dst, segment);
    }
    // Never answer a RST with a RST (RFC 9293 §3.10.7.1): two stacks would
    // bounce them back and forth.
    if segment.header.flags.contains(TcpFlags::RST) {
        return Ok(());
    }
    enqueue_reset(
        dst,
        src,
        segment.header.sequence,
        segment.header.acknowledgment,
    )
}

pub fn drain_outbound() -> Option<TcpOutboundSegment> {
    OUTBOUND.lock().pop_front()
}

#[cfg(feature = "neo-test")]
pub fn debug_outbound_len() -> usize {
    OUTBOUND.lock().len()
}

#[cfg(feature = "neo-test")]
pub fn debug_last_wire_egress() -> Option<TcpWireEgressCapture> {
    *LAST_WIRE_EGRESS.lock()
}

pub fn allocate_ephemeral_port() -> u16 {
    const FIRST: u32 = 49152;
    const COUNT: u32 = 65535 - FIRST + 1;
    let raw = NEXT_EPHEMERAL_PORT.fetch_add(1, Ordering::AcqRel);
    (FIRST + (raw % COUNT)) as u16
}

pub fn select_local_source(remote: SocketAddr, local_port: u16) -> Result<SocketAddr, TcpError> {
    if local_port == 0 {
        return Err(TcpError::Invalid);
    }
    let mut local = if is_loopback(&remote) {
        loopback_source(remote)
    } else {
        interface::select_source(remote).ok_or(TcpError::WouldBlock)?
    };
    local.port = local_port;
    Ok(local)
}

pub fn transition(state: TcpState, event: TcpEvent) -> Option<TcpState> {
    match (state, event) {
        (TcpState::Closed, TcpEvent::PassiveOpen) => Some(TcpState::Listen),
        (TcpState::Closed, TcpEvent::ActiveOpen) => Some(TcpState::SynSent),
        (TcpState::Listen, TcpEvent::ReceiveSyn) => Some(TcpState::SynReceived),
        (TcpState::Listen, TcpEvent::Close) => Some(TcpState::Closed),
        (TcpState::SynSent, TcpEvent::ReceiveSynAck) => Some(TcpState::Established),
        (TcpState::SynSent, TcpEvent::ReceiveSyn) => Some(TcpState::SynReceived),
        (TcpState::SynReceived, TcpEvent::ReceiveAck) => Some(TcpState::Established),
        (TcpState::Established, TcpEvent::Close) => Some(TcpState::FinWait1),
        (TcpState::Established, TcpEvent::ReceiveFin) => Some(TcpState::CloseWait),
        (TcpState::FinWait1, TcpEvent::ReceiveAck) => Some(TcpState::FinWait2),
        (TcpState::FinWait1, TcpEvent::ReceiveFin) => Some(TcpState::Closing),
        (TcpState::FinWait2, TcpEvent::ReceiveFin) => Some(TcpState::TimeWait),
        (TcpState::CloseWait, TcpEvent::Close) => Some(TcpState::LastAck),
        (TcpState::Closing, TcpEvent::ReceiveAck) => Some(TcpState::TimeWait),
        (TcpState::LastAck, TcpEvent::ReceiveAck) => Some(TcpState::Closed),
        (TcpState::TimeWait, TcpEvent::Timeout2Msl) => Some(TcpState::Closed),
        (_, TcpEvent::ReceiveRst) => Some(TcpState::Closed),
        _ => None,
    }
}

pub fn build_segment(
    src_ip: IpAddress,
    dst_ip: IpAddress,
    segment: TcpBuildSegment<'_>,
    out: &mut [u8],
) -> Result<usize, TcpError> {
    if !same_family(src_ip, dst_ip) {
        return Err(TcpError::Invalid);
    }
    if segment.options.len() > TCP_MAX_OPTION_LEN || !segment.options.len().is_multiple_of(4) {
        return Err(TcpError::Invalid);
    }
    let header_len = TCP_HEADER_LEN
        .checked_add(segment.options.len())
        .ok_or(TcpError::BufferTooSmall)?;
    let total_len = header_len
        .checked_add(segment.payload.len())
        .ok_or(TcpError::BufferTooSmall)?;
    if total_len > u16::MAX as usize || out.len() < total_len {
        return Err(TcpError::BufferTooSmall);
    }

    out[..total_len].fill(0);
    out[0..2].copy_from_slice(&segment.header.src_port.to_be_bytes());
    out[2..4].copy_from_slice(&segment.header.dst_port.to_be_bytes());
    out[4..8].copy_from_slice(&segment.header.sequence.to_be_bytes());
    out[8..12].copy_from_slice(&segment.header.acknowledgment.to_be_bytes());
    out[12] = ((header_len / 4) as u8) << 4;
    if segment.header.flags.contains(TcpFlags::NS) {
        out[12] |= 1;
    }
    out[13] = (segment.header.flags.bits() & 0xff) as u8;
    out[14..16].copy_from_slice(&segment.header.window.to_be_bytes());
    out[16..18].copy_from_slice(&0u16.to_be_bytes());
    out[18..20].copy_from_slice(&segment.header.urgent_pointer.to_be_bytes());
    out[TCP_HEADER_LEN..header_len].copy_from_slice(segment.options);
    out[header_len..total_len].copy_from_slice(segment.payload);

    let checksum = checksum_for_addresses(src_ip, dst_ip, &out[..total_len])?;
    out[16..18].copy_from_slice(&checksum.to_be_bytes());
    Ok(total_len)
}

pub fn parse_segment<'a>(
    src_ip: IpAddress,
    dst_ip: IpAddress,
    packet: &'a [u8],
) -> Result<TcpSegment<'a>, TcpError> {
    if !same_family(src_ip, dst_ip) || packet.len() < TCP_HEADER_LEN {
        return Err(TcpError::Invalid);
    }
    let header_len = ((packet[12] >> 4) as usize) * 4;
    if !(TCP_HEADER_LEN..=TCP_MAX_HEADER_LEN).contains(&header_len) || packet.len() < header_len {
        return Err(TcpError::Invalid);
    }
    if checksum_for_addresses(src_ip, dst_ip, packet)? != 0 {
        return Err(TcpError::Checksum);
    }
    let flags = TcpFlags(((packet[12] as u16 & 1) << 8) | packet[13] as u16);
    Ok(TcpSegment {
        header: TcpHeader {
            src_port: u16::from_be_bytes([packet[0], packet[1]]),
            dst_port: u16::from_be_bytes([packet[2], packet[3]]),
            sequence: u32::from_be_bytes([packet[4], packet[5], packet[6], packet[7]]),
            acknowledgment: u32::from_be_bytes([packet[8], packet[9], packet[10], packet[11]]),
            flags,
            window: u16::from_be_bytes([packet[14], packet[15]]),
            urgent_pointer: u16::from_be_bytes([packet[18], packet[19]]),
        },
        options: &packet[TCP_HEADER_LEN..header_len],
        payload: &packet[header_len..],
    })
}

pub fn checksum_for_addresses(
    src_ip: IpAddress,
    dst_ip: IpAddress,
    tcp_segment: &[u8],
) -> Result<u16, TcpError> {
    if !same_family(src_ip, dst_ip) {
        return Err(TcpError::Invalid);
    }
    if tcp_segment.len() > u16::MAX as usize {
        return Err(TcpError::BufferTooSmall);
    }
    let mut checksum = Checksum::new();
    match (src_ip, dst_ip) {
        (IpAddress::V4(src), IpAddress::V4(dst)) => {
            checksum.add(&src);
            checksum.add(&dst);
            checksum.add(&[0, IPV4_PROTOCOL_TCP]);
            checksum.add(&(tcp_segment.len() as u16).to_be_bytes());
        }
        (IpAddress::V6(src), IpAddress::V6(dst)) => {
            checksum.add(&src);
            checksum.add(&dst);
            checksum.add(&(tcp_segment.len() as u32).to_be_bytes());
            checksum.add(&[0, 0, 0, IPV4_PROTOCOL_TCP]);
        }
        _ => return Err(TcpError::Invalid),
    }
    checksum.add(tcp_segment);
    Ok(checksum.finish())
}

pub fn ip_from_socket_addr(addr: SocketAddr) -> IpAddress {
    match addr.family {
        AddressFamily::Inet4 => {
            let mut out = [0u8; 4];
            out.copy_from_slice(&addr.addr[..4]);
            IpAddress::V4(out)
        }
        AddressFamily::Inet6 => IpAddress::V6(addr.addr),
    }
}

pub fn tick() {
    let now = CLOCK_TICKS.fetch_add(1, Ordering::AcqRel).wrapping_add(1);
    #[cfg(not(feature = "net-tick"))]
    TIME_WAITS
        .lock()
        .retain(|_tuple, entry| !time_reached(now, entry.expires_at));
    // #175 item 4: with the periodic tick, each table is visited a round of
    // at most `TICK_BUDGET` entries at a time, resuming where the last round
    // stopped, so one tick's work doesn't grow with the table.
    #[cfg(feature = "net-tick")]
    {
        let mut table = TIME_WAITS.lock();
        let expired = round(&table, &TIME_WAIT_CURSOR, |tuple, entry| {
            time_reached(now, entry.expires_at).then_some(*tuple)
        });
        for tuple in expired.into_iter().flatten() {
            table.remove(&tuple);
        }
    }
    #[cfg(not(feature = "net-tick"))]
    let listeners: Vec<Arc<TcpListener>> = LISTENERS.lock().values().cloned().collect();
    #[cfg(feature = "net-tick")]
    let listeners = round(&LISTENERS.lock(), &LISTENER_CURSOR, |_, l| l.clone());
    let ticks = timer_now();
    for listener in listeners {
        reap_half_open(&listener, ticks);
    }
    #[cfg(not(feature = "net-tick"))]
    let sockets: Vec<Arc<TcpSocket>> = CONNECTIONS.lock().values().cloned().collect();
    #[cfg(feature = "net-tick")]
    let sockets = round(&CONNECTIONS.lock(), &CONNECTION_CURSOR, |_, s| s.clone());
    for socket in sockets {
        let due = socket.retransmit_due(now);
        for segment in due {
            let _ = enqueue_tcp_segment(
                socket.tuple,
                segment.flags,
                segment.sequence,
                segment.acknowledgment,
                &segment.payload,
            );
        }
    }
    #[cfg(not(feature = "net-tick"))]
    flush_wire_egress();
    // OT-96 F2 / #175 item 4: bounded with the periodic tick; the next tick
    // (and with `wire-rx`, `net-rx`'s passes) sends the rest.
    #[cfg(feature = "net-tick")]
    let _ = flush_wire_egress_some(WIRE_FLUSH_BUDGET);
}

/// #175 item 4: entries of one table a tick visits.
#[cfg(feature = "net-tick")]
pub const TICK_BUDGET: usize = 64;
/// OT-96 F2 / #175 item 4: wire segments one bounded flush looks at.
#[cfg(feature = "net-tick")]
pub const WIRE_FLUSH_BUDGET: usize = 16;
/// #175 item 4: segments one bounded loopback pump looks at.
#[cfg(feature = "net-tick")]
pub const PUMP_BUDGET: usize = 128;

#[cfg(feature = "net-tick")]
static TIME_WAIT_CURSOR: Mutex<Option<FourTuple>> = Mutex::new(None);
#[cfg(feature = "net-tick")]
static LISTENER_CURSOR: Mutex<Option<ListenKey>> = Mutex::new(None);
#[cfg(feature = "net-tick")]
static CONNECTION_CURSOR: Mutex<Option<FourTuple>> = Mutex::new(None);

/// #175 item 4: one tick's round over `map` (`egress_logic::round`).
#[cfg(feature = "net-tick")]
fn round<K: Ord + Copy, V, R>(
    map: &BTreeMap<K, V>,
    cursor: &Mutex<Option<K>>,
    f: impl FnMut(&K, &V) -> R,
) -> Vec<R> {
    crate::net::egress_logic::round(map, &mut cursor.lock(), TICK_BUDGET, f)
}

/// #175 item 4, Chad #189: [`pump_loopback_egress`] for the periodic tick,
/// bounded as a whole. Each round is one [`egress_logic::egress_pass`]: it
/// takes loopback segments off the front, delivers them, and puts any wire
/// segment back where it was; a delivery can queue replies, so rounds repeat
/// while one delivers, and all rounds together take no more than `budget`.
/// Its work is that budget's, whatever the backlog. A wire segment at the
/// front holds back the loopback ones behind it until a flush sends it (the
/// price of not walking the queue).
#[cfg(feature = "net-tick")]
pub fn pump_loopback_egress_some(budget: usize) {
    let mut left = budget;
    while left > 0 {
        let pass = crate::net::egress_logic::egress_pass(
            |f| f(&mut OUTBOUND.lock()),
            left,
            |segment| !is_loopback(&segment.dst),
            |segment| {
                let _ = inject_segment(segment.src, segment.dst, &segment.segment);
            },
        );
        left -= pass.taken;
        if pass.sent == 0 {
            break;
        }
    }
}

pub fn pump_loopback_egress() {
    for _ in 0..128 {
        let mut retained = VecDeque::new();
        let mut delivered = false;
        loop {
            let Some(segment) = OUTBOUND.lock().pop_front() else {
                break;
            };
            if is_loopback(&segment.dst) {
                let _ = inject_segment(segment.src, segment.dst, &segment.segment);
                delivered = true;
            } else {
                retained.push_back(segment);
            }
        }
        OUTBOUND.lock().extend(retained);
        if !delivered {
            break;
        }
    }
}

impl TcpSocket {
    fn receive_segment(&self, segment: TcpSegment<'_>) -> Result<(), TcpError> {
        let flags = segment.header.flags;
        if flags.contains(TcpFlags::RST) {
            let mut inner = self.inner.lock();
            if inner.state == TcpState::Established && segment.header.sequence != inner.rcv_nxt {
                let challenge = inner.challenge_ack_allowed(now());
                let seq = inner.snd_nxt;
                let ack = inner.rcv_nxt;
                drop(inner);
                if challenge {
                    enqueue_tcp_segment(self.tuple, TcpFlags::ACK, seq, ack, &[])?;
                }
                return Ok(());
            }
            let was_half_open = inner.state == TcpState::SynReceived;
            inner.state = TcpState::Closed;
            drop(inner);
            // A peer that resets its half-open connection frees the slot now,
            // not at the timeout.
            if was_half_open
                && let Some(listener) = lookup_listener_for(self.tuple.local)
                && let Some(socket) = listener.take_half_open(self)
            {
                close_connection(&socket);
            }
            return Ok(());
        }
        let mut accept_ready = false;
        let mut ack_to_send = None;
        {
            let mut inner = self.inner.lock();
            if flags.contains(TcpFlags::ACK) {
                if seq_gt(segment.header.acknowledgment, inner.snd_nxt) {
                    let challenge = inner.challenge_ack_allowed(now());
                    let seq = inner.snd_nxt;
                    let ack = inner.rcv_nxt;
                    drop(inner);
                    if challenge {
                        enqueue_tcp_segment(self.tuple, TcpFlags::ACK, seq, ack, &[])?;
                    }
                    return Ok(());
                }
                let old_snd_una = inner.snd_una;
                if seq_gt(segment.header.acknowledgment, inner.snd_una) {
                    inner.snd_una = segment.header.acknowledgment;
                    let acked = inner.snd_una.wrapping_sub(old_snd_una);
                    inner.cubic.on_ack(acked, now());
                }
                while matches!(inner.unacked.front(), Some(first) if seq_le(first.end_sequence, inner.snd_una))
                {
                    inner.unacked.pop_front();
                }
            }
            match inner.state {
                TcpState::SynSent => {
                    if !flags.contains(TcpFlags::SYN) || !flags.contains(TcpFlags::ACK) {
                        return Err(TcpError::Invalid);
                    }
                    if segment.header.acknowledgment != inner.snd_nxt {
                        return Err(TcpError::Invalid);
                    }
                    inner.irs = segment.header.sequence;
                    inner.rcv_nxt = segment.header.sequence.wrapping_add(1);
                    inner.snd_una = segment.header.acknowledgment;
                    inner.peer_window_scale = parse_window_scale(segment.options).unwrap_or(0);
                    inner.state = TcpState::Established;
                    ack_to_send = Some((inner.snd_nxt, inner.rcv_nxt));
                }
                TcpState::SynReceived => {
                    if !flags.contains(TcpFlags::ACK)
                        || segment.header.acknowledgment != inner.snd_nxt
                    {
                        return Err(TcpError::Invalid);
                    }
                    inner.snd_una = segment.header.acknowledgment;
                    inner.state = TcpState::Established;
                    accept_ready = true;
                }
                TcpState::Established => {
                    if !segment.payload.is_empty() {
                        if segment.header.sequence != inner.rcv_nxt {
                            return Err(TcpError::Invalid);
                        }
                        let payload = if let Some(tls) = inner.tls.as_mut() {
                            match tls.open_application_records(segment.payload) {
                                Ok(plaintext) => plaintext,
                                Err(_) => {
                                    inner.state = TcpState::Closed;
                                    return Err(TcpError::Invalid);
                                }
                            }
                        } else {
                            segment.payload.to_vec()
                        };
                        if inner.rx_queue.len().saturating_add(payload.len()) > RX_QUEUE_LIMIT {
                            return Err(TcpError::WouldBlock);
                        }
                        for byte in payload {
                            inner.rx_queue.push_back(byte);
                        }
                        inner.rcv_nxt = inner.rcv_nxt.wrapping_add(segment.payload.len() as u32);
                        ack_to_send = Some((inner.snd_nxt, inner.rcv_nxt));
                    }
                    if flags.contains(TcpFlags::FIN) {
                        if segment.header.sequence != inner.rcv_nxt {
                            return Err(TcpError::Invalid);
                        }
                        inner.rcv_nxt = inner.rcv_nxt.wrapping_add(1);
                        inner.state = TcpState::CloseWait;
                        ack_to_send = Some((inner.snd_nxt, inner.rcv_nxt));
                    }
                }
                TcpState::FinWait1 => {
                    if flags.contains(TcpFlags::ACK) && seq_le(inner.snd_nxt, inner.snd_una) {
                        inner.state = TcpState::FinWait2;
                    }
                    if flags.contains(TcpFlags::FIN) {
                        if segment.header.sequence != inner.rcv_nxt {
                            return Err(TcpError::Invalid);
                        }
                        inner.rcv_nxt = inner.rcv_nxt.wrapping_add(1);
                        inner.state = if flags.contains(TcpFlags::ACK)
                            && seq_le(inner.snd_nxt, inner.snd_una)
                        {
                            TcpState::TimeWait
                        } else {
                            TcpState::Closing
                        };
                        ack_to_send = Some((inner.snd_nxt, inner.rcv_nxt));
                    }
                }
                TcpState::FinWait2 => {
                    if flags.contains(TcpFlags::FIN) {
                        if segment.header.sequence != inner.rcv_nxt {
                            return Err(TcpError::Invalid);
                        }
                        inner.rcv_nxt = inner.rcv_nxt.wrapping_add(1);
                        inner.state = TcpState::TimeWait;
                        ack_to_send = Some((inner.snd_nxt, inner.rcv_nxt));
                    }
                }
                TcpState::Closing => {
                    if flags.contains(TcpFlags::ACK) && seq_le(inner.snd_nxt, inner.snd_una) {
                        inner.state = TcpState::TimeWait;
                    }
                }
                TcpState::LastAck => {
                    if flags.contains(TcpFlags::ACK) && seq_le(inner.snd_nxt, inner.snd_una) {
                        inner.state = TcpState::Closed;
                    }
                }
                _ => return Err(TcpError::Invalid),
            }
            if inner.state == TcpState::TimeWait {
                insert_time_wait(self.tuple, self.owner);
            }
        }
        if let Some((seq, ack)) = ack_to_send {
            enqueue_tcp_segment(self.tuple, TcpFlags::ACK, seq, ack, &[])?;
        }
        if accept_ready {
            let listener = lookup_listener_for(self.tuple.local).ok_or(TcpError::Invalid)?;
            // Not in the half-open queue: it was reaped meanwhile, and the
            // reaper closes it.
            let Some(socket) = listener.take_half_open(self) else {
                return Ok(());
            };
            #[cfg(feature = "neo-test-tcp-listen-bounds")]
            race_point(RaceAt::BeforeEnqueue);
            // The accept queue filled while this handshake ran, or the
            // listener was closed (#202 F1): refuse the connection rather than
            // keep it with no handle and no queue.
            if let Err(e) = listener.enqueue_accept(socket.clone()) {
                SYNS_REFUSED.fetch_add(1, Ordering::Relaxed);
                reset_and_close(&socket);
                return Err(e);
            }
        }
        Ok(())
    }

    fn retransmit_due(&self, now: u32) -> Vec<UnackedRetransmit> {
        let mut due = Vec::new();
        let mut inner = self.inner.lock();
        let mut loss = false;
        for segment in inner.unacked.iter_mut() {
            if time_reached(now, segment.retransmit_at) {
                due.push(UnackedRetransmit {
                    sequence: segment.sequence,
                    acknowledgment: segment.acknowledgment,
                    flags: segment.flags,
                    payload: segment.payload.clone(),
                });
                segment.retransmit_at = now.wrapping_add(RTO_TICKS);
                loss = true;
            }
        }
        if loss {
            inner.cubic.on_loss();
        }
        due
    }
}

impl TcpSocketInner {
    fn challenge_ack_allowed(&mut self, now: u32) -> bool {
        if time_reached(now, self.challenge_ack_after) {
            self.challenge_ack_after = now.wrapping_add(1);
            self.challenge_acks = self.challenge_acks.saturating_add(1);
            return true;
        }
        false
    }
}

struct UnackedRetransmit {
    sequence: u32,
    acknowledgment: u32,
    flags: TcpFlags,
    payload: Vec<u8>,
}

impl CubicState {
    fn new() -> Self {
        Self {
            cwnd: CUBIC_INITIAL_WINDOW,
            ssthresh: u32::MAX,
            w_max: CUBIC_INITIAL_WINDOW,
            epoch_start: None,
            origin_point: CUBIC_INITIAL_WINDOW,
            k_ticks: 0,
        }
    }

    fn on_ack(&mut self, acked: u32, now: u32) {
        if acked == 0 {
            return;
        }
        if self.cwnd < self.ssthresh {
            self.cwnd = self.cwnd.saturating_add(core::cmp::min(acked, TCP_MSS));
            return;
        }
        if self.epoch_start.is_none() {
            self.epoch_start = Some(now);
            self.origin_point = if self.cwnd < self.w_max {
                self.w_max
            } else {
                self.cwnd
            };
            self.k_ticks = cubic_k_ticks(self.w_max.saturating_sub(self.cwnd));
        }
        let elapsed = now.wrapping_sub(self.epoch_start.unwrap_or(now));
        let target = cubic_target(self.origin_point, elapsed as i64 - self.k_ticks as i64);
        if target > self.cwnd {
            self.cwnd = core::cmp::min(target, self.cwnd.saturating_add(TCP_MSS));
        } else {
            let increment = core::cmp::max(1, TCP_MSS.saturating_mul(TCP_MSS) / self.cwnd);
            self.cwnd = self.cwnd.saturating_add(increment);
        }
    }

    fn on_loss(&mut self) {
        self.w_max = self.cwnd;
        self.cwnd = core::cmp::max(
            TCP_MSS * 2,
            self.cwnd.saturating_mul(CUBIC_BETA_NUM) / CUBIC_BETA_DEN,
        );
        self.ssthresh = self.cwnd;
        self.epoch_start = None;
        self.origin_point = self.cwnd;
        self.k_ticks = 0;
    }

    fn snapshot(&self) -> CubicSnapshot {
        CubicSnapshot {
            cwnd: self.cwnd,
            ssthresh: self.ssthresh,
            w_max: self.w_max,
        }
    }
}

fn passive_open(
    remote: SocketAddr,
    local: SocketAddr,
    segment: TcpSegment<'_>,
) -> Result<(), TcpError> {
    let listener = lookup_listener_for(local).ok_or(TcpError::Invalid)?;
    #[cfg(feature = "neo-test-tcp-listen-bounds")]
    race_point(RaceAt::AfterLookup);
    let tuple = FourTuple::new(local, remote)?;
    if lookup_connection(tuple).is_some() {
        return Err(TcpError::Busy);
    }
    // OT-94: at most `backlog` half-open connections per listener, and none
    // while the accept queue is full. A refused SYN gets a RST and allocates
    // nothing (smoltcp's rule for a SYN no listening socket can take).
    let now = timer_now();
    reap_half_open(&listener, now);
    if listener.accept_full() || listener.half_open_count() >= listener.backlog() {
        SYNS_REFUSED.fetch_add(1, Ordering::Relaxed);
        return enqueue_reset(
            local,
            remote,
            segment.header.sequence,
            segment.header.acknowledgment,
        );
    }
    let iss = next_iss();
    let socket = Arc::new(TcpSocket::new(
        listener.owner(),
        tuple,
        TcpState::SynReceived,
    ));
    {
        let mut inner = socket.inner.lock();
        inner.iss = iss;
        inner.irs = segment.header.sequence;
        inner.snd_una = iss;
        inner.snd_nxt = iss.wrapping_add(1);
        inner.rcv_nxt = segment.header.sequence.wrapping_add(1);
        inner.peer_window_scale = parse_window_scale(segment.options).unwrap_or(0);
    }
    // Two CPUs can pass the check above for the same listener; the
    // reservation is the authoritative one.
    if !listener.reserve_half_open(&socket, now) {
        SYNS_REFUSED.fetch_add(1, Ordering::Relaxed);
        return enqueue_reset(
            local,
            remote,
            segment.header.sequence,
            segment.header.acknowledgment,
        );
    }
    #[cfg(feature = "neo-test-tcp-listen-bounds")]
    race_point(RaceAt::BeforeInsert);
    if let Err(e) = insert_connection(socket.clone()) {
        listener.take_half_open(Arc::as_ptr(&socket));
        return Err(e);
    }
    // #202 F1 / #170: the listener closed after the reservation. If its drain
    // already ran, it reset this socket before the insert above, so take it
    // out of `CONNECTIONS` here; if it is still queued, whoever takes it from
    // the queue (this, or the drain) resets and closes it.
    if listener.is_retired() {
        match listener.take_half_open(Arc::as_ptr(&socket)) {
            Some(socket) => reset_and_close(&socket),
            None => close_connection(&socket),
        }
        return Ok(());
    }
    enqueue_tcp_segment_with_options(
        tuple,
        TcpFlags::SYN.union(TcpFlags::ACK),
        iss,
        segment.header.sequence.wrapping_add(1),
        &TCP_WINDOW_SCALE_OPTION,
        &[],
    )
}

fn lookup_listener_for(local: SocketAddr) -> Option<Arc<TcpListener>> {
    let table = LISTENERS.lock();
    table.get(&listen_key(local)).cloned().or_else(|| {
        table
            .get(&ListenKey {
                family: local.family,
                port: local.port,
                addr: [0; 16],
            })
            .cloned()
    })
}

fn insert_time_wait(tuple: FourTuple, owner: ProcessId) {
    TIME_WAITS.lock().insert(
        tuple,
        TimeWaitEntry {
            owner,
            expires_at: now().wrapping_add(TIME_WAIT_TICKS),
        },
    );
}

fn enqueue_reset(
    src: SocketAddr,
    dst: SocketAddr,
    sequence: u32,
    acknowledgment: u32,
) -> Result<(), TcpError> {
    let tuple = FourTuple::new(src, dst)?;
    enqueue_tcp_segment(
        tuple,
        TcpFlags::RST,
        acknowledgment,
        sequence.wrapping_add(1),
        &[],
    )
}

fn enqueue_tcp_segment(
    tuple: FourTuple,
    flags: TcpFlags,
    sequence: u32,
    acknowledgment: u32,
    payload: &[u8],
) -> Result<(), TcpError> {
    enqueue_tcp_segment_with_options(tuple, flags, sequence, acknowledgment, &[], payload)
}

fn enqueue_tcp_segment_with_options(
    tuple: FourTuple,
    flags: TcpFlags,
    sequence: u32,
    acknowledgment: u32,
    options: &[u8],
    payload: &[u8],
) -> Result<(), TcpError> {
    let mut bytes = vec![0u8; TCP_MAX_HEADER_LEN.saturating_add(payload.len())];
    let len = build_segment(
        ip_from_socket_addr(tuple.local),
        ip_from_socket_addr(tuple.remote),
        TcpBuildSegment {
            header: TcpHeader {
                src_port: tuple.local.port,
                dst_port: tuple.remote.port,
                sequence,
                acknowledgment,
                flags,
                window: DEFAULT_WINDOW,
                urgent_pointer: 0,
            },
            options,
            payload,
        },
        &mut bytes,
    )?;
    bytes.truncate(len);
    let segment = TcpOutboundSegment {
        src: tuple.local,
        dst: tuple.remote,
        flags,
        sequence,
        acknowledgment,
        segment: bytes,
    };
    #[cfg(not(feature = "net-tick"))]
    OUTBOUND.lock().push_back(segment);
    // Chad #189: an admission threshold with the periodic tick. A segment
    // past it is dropped and counted, as a full NIC queue drops a frame: TCP recovers by
    // retransmission (data and FIN stay in `unacked`), and a dropped RST or
    // ACK is one the peer's own retransmission draws again.
    #[cfg(feature = "net-tick")]
    if !crate::net::egress_logic::push_capped(&mut OUTBOUND.lock(), OUTBOUND_ADMIT, segment) {
        OUTBOUND_DROPS.fetch_add(1, Ordering::Relaxed);
    }
    Ok(())
}

pub(crate) fn flush_wire_egress() {
    let mut retained = VecDeque::new();
    loop {
        let Some(segment) = OUTBOUND.lock().pop_front() else {
            break;
        };
        if is_loopback(&segment.dst) {
            retained.push_back(segment);
            continue;
        }
        let _ = transmit_wire_segment(&segment);
    }
    OUTBOUND.lock().extend(retained);
}

/// OT-96 F2 (Chad #173), #175 item 4, Chad #189: [`flush_wire_egress`],
/// bounded as a whole: one [`egress_logic::egress_pass`]. It takes at most
/// `budget` segments off the front of the queue, sends the wire ones, and
/// puts the loopback ones back where they were; nothing else in the queue is
/// touched, so its work is the budget's whatever the backlog or what other
/// CPUs queue meanwhile (the old drain ran until the queue was empty, and the
/// first bounded one still rebuilt the rest). A loopback segment at the front
/// holds back the wire ones behind it until the pump delivers it. Returns
/// whether segments remain.
#[cfg(feature = "net-tick")]
pub(crate) fn flush_wire_egress_some(budget: usize) -> bool {
    crate::net::egress_logic::egress_pass(
        |f| f(&mut OUTBOUND.lock()),
        budget,
        |segment| is_loopback(&segment.dst),
        |segment| {
            let _ = transmit_wire_segment(&segment);
        },
    )
    .more
}

fn transmit_wire_segment(segment: &TcpOutboundSegment) -> Result<(), TcpError> {
    match segment.src.family {
        AddressFamily::Inet4 => transmit_ipv4_segment(segment),
        AddressFamily::Inet6 => transmit_ipv6_segment(segment),
    }
}

fn transmit_ipv4_segment(segment: &TcpOutboundSegment) -> Result<(), TcpError> {
    // ISC-3: egress on the interface that owns the source (interface 0 when
    // `interface-identity` is off — bit-for-bit the old `get_device(0)`).
    let iface = crate::net::interface::egress_interface_for_source(&segment.src);
    let device = net::get_device(iface as usize).ok_or(TcpError::WouldBlock)?;
    let src_ip = ipv4_addr(segment.src);
    let dst_ip = ipv4_addr(segment.dst);
    let frame_len = ETHERNET_HEADER_LEN
        .checked_add(ipv4::IPV4_HEADER_LEN)
        .and_then(|len| len.checked_add(segment.segment.len()))
        .ok_or(TcpError::BufferTooSmall)?;
    let mut frame = vec![0u8; frame_len];
    frame[6..12].copy_from_slice(&device.mac_address());
    frame[12..14].copy_from_slice(&ipv4::ETHERNET_IPV4.to_be_bytes());
    let header = ipv4::Ipv4Header {
        src: src_ip,
        dst: dst_ip,
        protocol: IPV4_PROTOCOL_TCP,
        ttl: ipv4::DEFAULT_TTL,
        identification: 0,
        payload_len: segment.segment.len() as u16,
    };
    header
        .encode(&mut frame[ETHERNET_HEADER_LEN..])
        .map_err(|_| TcpError::Invalid)?;
    frame[ETHERNET_HEADER_LEN + ipv4::IPV4_HEADER_LEN..].copy_from_slice(&segment.segment);
    *LAST_WIRE_EGRESS.lock() = Some(TcpWireEgressCapture {
        src: segment.src,
        dst: segment.dst,
        frame_len,
        ip_version: 4,
        protocol: IPV4_PROTOCOL_TCP,
    });
    if let Some(dst_mac) = arp::lookup(dst_ip) {
        frame[..6].copy_from_slice(&dst_mac);
        submit_frame(&frame, iface)
    } else {
        arp::submit_or_queue_ipv4(iface, src_ip, dst_ip, frame).map_err(|_| TcpError::WouldBlock)
    }
}

fn transmit_ipv6_segment(segment: &TcpOutboundSegment) -> Result<(), TcpError> {
    // ISC-3: egress on the interface that owns the source (interface 0 when
    // `interface-identity` is off — bit-for-bit the old `get_device(0)`).
    let iface = crate::net::interface::egress_interface_for_source(&segment.src);
    let device = net::get_device(iface as usize).ok_or(TcpError::WouldBlock)?;
    let frame_len = ETHERNET_HEADER_LEN
        .checked_add(IPV6_HEADER_LEN)
        .and_then(|len| len.checked_add(segment.segment.len()))
        .ok_or(TcpError::BufferTooSmall)?;
    if segment.segment.len() > u16::MAX as usize {
        return Err(TcpError::BufferTooSmall);
    }
    let mut frame = vec![0u8; frame_len];
    frame[6..12].copy_from_slice(&device.mac_address());
    frame[12..14].copy_from_slice(&ETHERNET_IPV6.to_be_bytes());
    let ip = ETHERNET_HEADER_LEN;
    frame[ip] = 0x60;
    frame[ip + 4..ip + 6].copy_from_slice(&(segment.segment.len() as u16).to_be_bytes());
    frame[ip + 6] = IPV4_PROTOCOL_TCP;
    frame[ip + 7] = DEFAULT_HOP_LIMIT;
    frame[ip + 8..ip + 24].copy_from_slice(&segment.src.addr);
    frame[ip + 24..ip + 40].copy_from_slice(&segment.dst.addr);
    frame[ip + IPV6_HEADER_LEN..].copy_from_slice(&segment.segment);
    *LAST_WIRE_EGRESS.lock() = Some(TcpWireEgressCapture {
        src: segment.src,
        dst: segment.dst,
        frame_len,
        ip_version: 6,
        protocol: IPV4_PROTOCOL_TCP,
    });
    ndp::submit_or_queue_ipv6(iface, segment.src.addr, segment.dst.addr, frame)
        .map_err(|_| TcpError::WouldBlock)
}

fn submit_frame(bytes: &[u8], iface: u32) -> Result<(), TcpError> {
    let device = net::get_device(iface as usize).ok_or(TcpError::WouldBlock)?;
    let mut frame = device.tx_reserve().ok_or(TcpError::WouldBlock)?;
    if bytes.len() > frame.len() {
        return Err(TcpError::BufferTooSmall);
    }
    frame.as_mut_slice()[..bytes.len()].copy_from_slice(bytes);
    device
        .tx_submit(frame, bytes.len())
        .map_err(|_| TcpError::WouldBlock)
}

fn listen_key(addr: SocketAddr) -> ListenKey {
    ListenKey {
        family: addr.family,
        port: addr.port,
        addr: addr.addr,
    }
}

fn next_iss() -> u32 {
    NEXT_ISS.fetch_add(64000, Ordering::AcqRel)
}

fn now() -> u32 {
    CLOCK_TICKS.load(Ordering::Acquire)
}

fn seq_le(a: u32, b: u32) -> bool {
    (a == b) || ((a.wrapping_sub(b) as i32) < 0)
}

fn seq_gt(a: u32, b: u32) -> bool {
    (a != b) && ((a.wrapping_sub(b) as i32) > 0)
}

fn time_reached(now: u32, deadline: u32) -> bool {
    (now.wrapping_sub(deadline) as i32) >= 0
}

fn bytes_in_flight(inner: &TcpSocketInner) -> u32 {
    inner.snd_nxt.wrapping_sub(inner.snd_una)
}

fn ipv4_addr(addr: SocketAddr) -> [u8; 4] {
    let mut out = [0u8; 4];
    out.copy_from_slice(&addr.addr[..4]);
    out
}

fn cubic_k_ticks(distance: u32) -> u32 {
    let mut k = 0u32;
    while cubic_scaled(k as i64) < distance as i64 && k < 4096 {
        k = k.saturating_add(1);
    }
    k
}

fn cubic_target(origin: u32, t_minus_k: i64) -> u32 {
    let delta = cubic_scaled(t_minus_k);
    if delta >= 0 {
        origin.saturating_add(delta as u32)
    } else {
        origin.saturating_sub(delta.unsigned_abs() as u32)
    }
}

fn cubic_scaled(x: i64) -> i64 {
    let cube = x.saturating_mul(x).saturating_mul(x);
    cube.saturating_mul(CUBIC_SCALE_NUM) / CUBIC_SCALE_DEN
}

fn parse_window_scale(options: &[u8]) -> Option<u8> {
    let mut idx = 0;
    while idx < options.len() {
        match options[idx] {
            TCP_OPTION_END => return None,
            TCP_OPTION_NOP => idx += 1,
            TCP_OPTION_WINDOW_SCALE => {
                if idx + 3 <= options.len() && options[idx + 1] == 3 {
                    return Some(core::cmp::min(options[idx + 2], 14));
                }
                return None;
            }
            _ => {
                if idx + 1 >= options.len() || options[idx + 1] < 2 {
                    return None;
                }
                idx += options[idx + 1] as usize;
            }
        }
    }
    None
}

fn is_loopback(addr: &SocketAddr) -> bool {
    match addr.family {
        AddressFamily::Inet4 => addr.addr[..4] == [127, 0, 0, 1],
        AddressFamily::Inet6 => addr.addr == [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
    }
}

fn loopback_source(remote: SocketAddr) -> SocketAddr {
    let mut addr = [0u8; 16];
    match remote.family {
        AddressFamily::Inet4 => addr[..4].copy_from_slice(&[127, 0, 0, 1]),
        AddressFamily::Inet6 => addr = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
    }
    SocketAddr {
        family: remote.family,
        port: 0,
        addr,
    }
}

fn same_family(src: IpAddress, dst: IpAddress) -> bool {
    matches!(
        (src, dst),
        (IpAddress::V4(_), IpAddress::V4(_)) | (IpAddress::V6(_), IpAddress::V6(_))
    )
}

struct Checksum {
    sum: u32,
    odd: Option<u8>,
}

impl Checksum {
    fn new() -> Self {
        Self { sum: 0, odd: None }
    }

    fn add(&mut self, bytes: &[u8]) {
        let mut idx = 0;
        if let Some(high) = self.odd.take() {
            if bytes.is_empty() {
                self.odd = Some(high);
                return;
            }
            self.sum = self
                .sum
                .wrapping_add(u16::from_be_bytes([high, bytes[0]]) as u32);
            idx = 1;
        }
        while idx + 1 < bytes.len() {
            self.sum = self
                .sum
                .wrapping_add(u16::from_be_bytes([bytes[idx], bytes[idx + 1]]) as u32);
            idx += 2;
        }
        if idx < bytes.len() {
            self.odd = Some(bytes[idx]);
        }
    }

    fn finish(mut self) -> u16 {
        if let Some(high) = self.odd {
            self.sum = self.sum.wrapping_add((high as u32) << 8);
        }
        while self.sum >> 16 != 0 {
            self.sum = (self.sum & 0xffff) + (self.sum >> 16);
        }
        !(self.sum as u16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sched::process::ProcessId;

    fn v4_addr(port: u16, addr: [u8; 4]) -> SocketAddr {
        let mut full = [0u8; 16];
        full[..4].copy_from_slice(&addr);
        SocketAddr {
            family: AddressFamily::Inet4,
            port,
            addr: full,
        }
    }

    fn v6_addr(port: u16, addr: [u8; 16]) -> SocketAddr {
        SocketAddr {
            family: AddressFamily::Inet6,
            port,
            addr,
        }
    }

    fn header(flags: TcpFlags) -> TcpHeader {
        TcpHeader {
            src_port: 49152,
            dst_port: 80,
            sequence: 0x1122_3344,
            acknowledgment: 0x5566_7788,
            flags,
            window: 4096,
            urgent_pointer: 0,
        }
    }

    #[test]
    fn segment_round_trip_checksum_ipv4() {
        let src = IpAddress::V4([192, 0, 2, 10]);
        let dst = IpAddress::V4([198, 51, 100, 20]);
        let options = [1u8, 1, 1, 1];
        let payload = [b'n', b'e', b'o'];
        let mut packet = [0u8; 64];
        let built = build_segment(
            src,
            dst,
            TcpBuildSegment {
                header: header(TcpFlags::SYN.union(TcpFlags::ACK)),
                options: &options,
                payload: &payload,
            },
            &mut packet,
        );
        assert_eq!(built, Ok(27));
        assert_eq!(checksum_for_addresses(src, dst, &packet[..27]), Ok(0));
        let parsed = parse_segment(src, dst, &packet[..27]);
        assert!(
            matches!(parsed, Ok(segment) if segment.header == header(TcpFlags::SYN.union(TcpFlags::ACK)) && segment.options == options && segment.payload == payload)
        );
    }

    #[test]
    fn segment_round_trip_checksum_ipv6() {
        let src = IpAddress::V6([0x20, 1, 0xd, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        let dst = IpAddress::V6([0x20, 1, 0xd, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2]);
        let payload = [1u8, 2, 3, 4, 5];
        let mut packet = [0u8; 64];
        let built = build_segment(
            src,
            dst,
            TcpBuildSegment {
                header: header(TcpFlags::ACK),
                options: &[],
                payload: &payload,
            },
            &mut packet,
        );
        assert_eq!(built, Ok(25));
        assert_eq!(checksum_for_addresses(src, dst, &packet[..25]), Ok(0));
        let parsed = parse_segment(src, dst, &packet[..25]);
        assert!(
            matches!(parsed, Ok(segment) if segment.header == header(TcpFlags::ACK) && segment.payload == payload)
        );
    }

    #[test]
    fn state_table_and_connection_table() {
        let local = v4_addr(4000, [127, 0, 0, 1]);
        let remote = v4_addr(5000, [127, 0, 0, 1]);
        let tuple = FourTuple::new(local, remote);
        assert!(matches!(tuple, Ok(_)));
        let tuple = tuple.ok().unwrap_or(FourTuple { local, remote });
        assert_eq!(
            transition(TcpState::Closed, TcpEvent::PassiveOpen),
            Some(TcpState::Listen)
        );
        assert_eq!(
            transition(TcpState::Listen, TcpEvent::ReceiveSyn),
            Some(TcpState::SynReceived)
        );
        assert_eq!(
            transition(TcpState::SynReceived, TcpEvent::ReceiveAck),
            Some(TcpState::Established)
        );
        let _ = remove_connection(tuple);
        let socket = Arc::new(TcpSocket::new(ProcessId(7), tuple, TcpState::SynReceived));
        assert_eq!(insert_connection(socket.clone()), Ok(()));
        assert!(matches!(lookup_connection(tuple), Some(found) if found.owner() == ProcessId(7)));
        assert_eq!(
            remove_connection(tuple).map(|s| s.state()),
            Some(TcpState::SynReceived)
        );
    }

    #[test]
    fn listener_queue_respects_backlog() {
        let local = v6_addr(8080, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        let remote = v6_addr(4000, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]);
        let tuple = FourTuple::new(local, remote);
        assert!(matches!(tuple, Ok(_)));
        let tuple = tuple.ok().unwrap_or(FourTuple { local, remote });
        let listener = TcpListener::new(ProcessId(9), local, 1);
        assert!(matches!(listener, Ok(_)));
        let listener = listener.ok().unwrap_or_else(|| TcpListener {
            owner: ProcessId(9),
            local,
            backlog: 1,
            accept_queue: Mutex::new(VecDeque::new()),
            half_open: Mutex::new(VecDeque::new()),
            retired: AtomicBool::new(false),
            handles: crate::net::HandleCount::default(),
        });
        let socket = Arc::new(TcpSocket::new(ProcessId(9), tuple, TcpState::Established));
        assert_eq!(listener.enqueue_accept(socket.clone()), Ok(()));
        assert_eq!(listener.enqueue_accept(socket), Err(TcpError::WouldBlock));
        assert!(
            matches!(listener.dequeue_accept(), Ok(accepted) if accepted.state() == TcpState::Established)
        );
    }
}
