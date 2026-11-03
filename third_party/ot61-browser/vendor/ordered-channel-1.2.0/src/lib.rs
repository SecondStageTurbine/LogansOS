//! Equivalent of mpsc or crossbeam-channel, but allows sending messages with an index associated with them,
//! to be delivered in the specific indexed order.
//!
//! This enables performing multi-threaded work without accidentally reordering results.
//!
//! Results are delivered as soon as possible, with minimal buffering.
//! Items received out of order are temporarily kept in a binary heap.

#![allow(clippy::match_same_arms)]
#[cfg(feature = "crossbeam-channel")]
/// Error type
///
/// Use these re-exported aliases, since they change depending on configured back-end.
///
pub use crossbeam_channel::{SendError, TrySendError, RecvError, TryRecvError};
#[cfg(feature = "crossbeam-channel")]
use crossbeam_channel::{
    Receiver as PlainReceiver,
    Sender as PlainSender,
    Sender as PlainSyncSender,
    bounded as plain_bounded,
    unbounded as plain_unbounded,
};

#[cfg(not(feature = "crossbeam-channel"))]
/// Error type
///
/// Use these re-exported aliases, since they change depending on configured back-end.
///
pub use std::sync::mpsc::{SendError, TrySendError, RecvError, TryRecvError};
#[cfg(not(feature = "crossbeam-channel"))]
use std::sync::mpsc::{
    Receiver as PlainReceiver,
    Sender as PlainSender,
    SyncSender as PlainSyncSender,
    sync_channel as plain_bounded,
    channel as plain_unbounded,
};

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::iter::FusedIterator;

enum SenderKind<T> {
    Bounded(PlainSyncSender<T>),
    Unbounded(PlainSender<T>),
}

/// A channel sender that orders messages by an index
///
/// It's cheap to clone
pub struct Sender<T> {
    sender: SenderKind<OrderByKey<T>>,
}

impl<T> Clone for Sender<T> {
    #[inline]
    fn clone(&self) -> Self {
        Self { sender: match &self.sender {
            SenderKind::Bounded(s) => SenderKind::Bounded(s.clone()),
            SenderKind::Unbounded(s) => SenderKind::Unbounded(s.clone()),
        } }
    }
}

/// Receiver that orders messages by an index
pub struct Receiver<T> {
    receiver: PlainReceiver<OrderByKey<T>>,
    next_index: usize,
    receive_buffer: BinaryHeap<OrderByKey<T>>,
}

impl<T> Receiver<T> {
    /// Gets a message with the next *consecutive* index
    ///
    /// Blocks until next message in order is received, or until all senders are dropped.
    /// Messages are never lost.
    pub fn recv(&mut self) -> Result<T, RecvError> {
        while self.receive_buffer.peek().map_or(true, |i| i.0 > self.next_index) {
            match self.receiver.recv() {
                Ok(OrderByKey(index, item)) if index <= self.next_index => {
                    self.next_index = self.next_index.max(index + 1);
                    return Ok(item);
                },
                Ok(queued) => {
                    self.receive_buffer.push(queued);
                },
                Err(_) => {
                    // Sender dropped (but continue to dump receive_buffer)
                    break;
                },
            }
        }

        let OrderByKey(index, item) = self.receive_buffer.pop()
            .ok_or(RecvError)?;
        self.next_index = self.next_index.max(index + 1);
        Ok(item)
    }

    /// Does not block, and returns immediately if there are no messages ready to take
    pub fn try_recv(&mut self) -> Result<T, TryRecvError> {
        while self.receive_buffer.peek().map_or(true, |i| i.0 > self.next_index) {
            match self.receiver.try_recv() {
                Ok(OrderByKey(index, item)) if index <= self.next_index => {
                    self.next_index = self.next_index.max(index + 1);
                    return Ok(item);
                },
                Ok(queued) => {
                    self.receive_buffer.push(queued);
                },
                Err(e @ TryRecvError::Empty) => {
                    return Err(e);
                },
                Err(TryRecvError::Disconnected) => {
                    // Sender dropped (but continue to dump receive_buffer)
                    break;
                },
            }
        }

        let OrderByKey(index, item) = self.receive_buffer.pop()
            .ok_or(TryRecvError::Disconnected)?;
        self.next_index = self.next_index.max(index + 1);
        Ok(item)
    }
}

/// Make a blocking channel with finite size
///
/// Returns `(sender, receiver)`
#[inline]
#[must_use]
pub fn bounded<T>(depth: usize) -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = plain_bounded(depth);
    make(SenderKind::Bounded(tx), rx)
}

/// Make a channel that can grow until the program runs out of memory
///
/// Returns `(sender, receiver)`
#[inline]
#[must_use]
pub fn unbounded<T>() -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = plain_unbounded();
    make(SenderKind::Unbounded(tx), rx)
}

#[inline]
fn make<T>(sender: SenderKind<OrderByKey<T>>, receiver: PlainReceiver<OrderByKey<T>>) -> (Sender<T>, Receiver<T>) {
    (Sender {
        sender,
    }, Receiver {
        receiver,
        next_index: 0,
        receive_buffer: BinaryHeap::new(),
    })
}

impl<T: Send> Sender<T> {
    /// It's important that indexes are consecutive and have no holes.
    /// Count starts at 0. Indexes from `iter().enumerate()` will work great.
    ///
    /// If any integer is missing, the receiver will wait for it until
    /// all senders are dropped.
    ///
    /// If any integer is sent more than once, only the first one received
    /// is going to be ordered, and the order of the remaining ones is undefined.
    #[inline]
    pub fn send(&self, index: usize, item: T) -> Result<(), SendError<T>> {
        match &self.sender {
            SenderKind::Bounded(s) => s.send(OrderByKey(index, item)),
            SenderKind::Unbounded(s) => s.send(OrderByKey(index, item)),
        }
        .map_err(|SendError(OrderByKey(_, e))| SendError(e))
    }

    /// Does not send if the channel is bounded and full.
    ///
    /// For unbounded channels it's the same as `send()`.
    #[inline]
    pub fn try_send(&self, index: usize, item: T) -> Result<(), TrySendError<T>> {
        match &self.sender {
            SenderKind::Bounded(s) => match s.try_send(OrderByKey(index, item)) {
                Ok(()) => Ok(()),
                Err(TrySendError::Full(OrderByKey(_, e))) => Err(TrySendError::Full(e)),
                Err(TrySendError::Disconnected(OrderByKey(_, e))) => Err(TrySendError::Disconnected(e)),
            },
            SenderKind::Unbounded(s) => match s.send(OrderByKey(index, item)) {
                Ok(()) => Ok(()),
                Err(SendError(OrderByKey(_, e))) => Err(TrySendError::Disconnected(e)),
            },
        }
    }
}

impl<T> FusedIterator for Receiver<T> {}

impl<T> Iterator for Receiver<T> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<T> {
        self.recv().ok()
    }
}

struct OrderByKey<T>(usize, T);
impl<T> PartialEq for OrderByKey<T> {
    #[inline]
    fn eq(&self, o: &Self) -> bool { o.0.eq(&self.0) }
}
impl<T> Eq for OrderByKey<T> {}
impl<T> PartialOrd for OrderByKey<T> {
    #[inline]
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> { Some(self.cmp(o)) }
}
impl<T> Ord for OrderByKey<T> {
    #[inline]
    fn cmp(&self, o: &Self) -> Ordering { o.0.cmp(&self.0) }
}

#[test]
fn test() {
    let (s, mut r) = bounded(10);
    s.send(1, "B").unwrap();
    s.send(0, "A").unwrap();
    s.send(200, "X").unwrap();
    s.send(0, "bad A").unwrap();
    std::thread::spawn(move || {
        s.send(2, "C").unwrap();
    });
    assert_eq!("A", r.recv().unwrap());
    assert_eq!("B", r.recv().unwrap());
    assert_eq!("bad A", r.recv().unwrap());
    assert_eq!("C", r.recv().unwrap());
    assert_eq!("X", r.recv().unwrap());
    assert!(r.recv().is_err());
}

#[test]
fn test_recovers_order() {
    let (s, mut r) = unbounded();
    s.send(1, "B").unwrap();
    s.send(0, "A").unwrap();
    assert_eq!("A", r.recv().unwrap());
    assert_eq!("B", r.recv().unwrap());

    s.send(3, "D").unwrap();
    s.send(0, "bad A").unwrap();
    s.send(2, "C").unwrap();
    assert_eq!("bad A", r.recv().unwrap());

    assert_eq!("C", r.recv().unwrap());
    drop(s);
    assert_eq!("D", r.recv().unwrap());
    assert!(r.recv().is_err());
}

#[test]
fn test_recovers_order_buffered() {
    let (s, mut r) = unbounded();
    s.send(3, "D").unwrap();
    s.send(1, "B").unwrap();
    s.send(4, "E").unwrap();
    s.send(1, "bad B").unwrap();
    s.send(0, "A").unwrap();
    assert_eq!("A", r.recv().unwrap());
    assert_eq!("B", r.recv().unwrap());
    assert_eq!("bad B", r.recv().unwrap());

    s.send(2, "C").unwrap();
    assert_eq!("C", r.recv().unwrap());
    drop(s);
    assert_eq!("D", r.recv().unwrap());
    assert_eq!("E", r.recv().unwrap());
    assert!(r.recv().is_err());
}

#[test]
fn test_try() {
    let (s, mut r) = bounded(10);
    s.try_send(1, "B").unwrap();
    s.try_send(0, "A").unwrap();
    s.try_send(2, "C").unwrap();

    assert_eq!("A", r.try_recv().unwrap());
    assert_eq!("B", r.try_recv().unwrap());
    assert_eq!("C", r.try_recv().unwrap());
    drop(s);
    assert!(r.try_recv().is_err());
    assert!(r.recv().is_err());
}
