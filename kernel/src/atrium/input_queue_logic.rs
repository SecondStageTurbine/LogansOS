// SPDX-License-Identifier: MPL-2.0
//! Per-client input queue policy: pointer motion can never evict a keystroke.
//!
//! ADR-074 routes keys and pointer events into one ordered, bounded queue per
//! `InputFocus`. The original overflow rule — drop the oldest event — let a
//! moving mouse push keystrokes out of the queue of a client that drains
//! slower than the mouse reports. This policy keeps the single ordered stream
//! (so key/click order is unchanged) and changes only *what* is merged or
//! evicted:
//!
//! 1. **Coalesce.** A motion-only pointer event (button state unchanged since
//!    the previous pointer event routed to this queue) merges into the tail if
//!    the tail is also motion-only with the same buttons: deltas add,
//!    saturating at `i16`. Merging into the tail never reorders anything.
//! 2. **Evict by class.** On overflow the oldest motion-only event goes first,
//!    then the oldest button transition that a later queued pointer event
//!    supersedes (each pointer event carries the full button bitmap, so the
//!    client's final button state stays correct), and only then the oldest key.
//!    An incoming motion-only event that finds nothing cheaper is dropped
//!    itself. So a key is evicted only by an incoming key, or by an incoming
//!    button change into a queue holding nothing but keys.
//!
//! With no pointer events at all, step 1 never applies and step 2 always takes
//! the oldest key, so the key path is bit-identical to the old queue (the
//! kill-switch/ISC property the host KAT asserts). Pure and syscall-free:
//! bounded O(capacity) work per enqueue, no allocation (the deque is created
//! with its capacity and never grows past it).

extern crate alloc;

use alloc::collections::VecDeque;

use super::input_logic::{INPUT_EVENT_POINTER, InputEventV1};
use super::pointer_route_logic::{INPUT_EVENT_POINTER_ABS, PHASE_MOVE};

/// Kind 2 (relative) or kind 3 (a WM-forwarded absolute event, DI-1a): both
/// carry the full button bitmap in `flags`, so both get the pointer policy.
fn is_pointer(event: &InputEventV1) -> bool {
    event.kind == INPUT_EVENT_POINTER || event.kind == INPUT_EVENT_POINTER_ABS
}

/// A9-C2: a raw pointer report that carries a wheel (`code`, kind 2 only:
/// kind 3 reuses `code` for its phase). Its wheel is not a delta to merge
/// and not superseded by later reports, so it is never coalesced and never
/// evicted as superseded; only the last-resort eviction below can take one,
/// from a full queue that holds nothing else. Like motion, it never evicts a
/// key or a transition: a full queue drops it instead, unless its buttons
/// changed. With no wheel (every report before A9-C2) nothing here applies.
fn carries_wheel(event: &InputEventV1) -> bool {
    event.kind == INPUT_EVENT_POINTER && event.code != 0
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Queued {
    event: InputEventV1,
    /// A pointer event whose buttons equal the previous pointer event's.
    motion_only: bool,
}

/// What `enqueue` did with the event (counted by the caller for telemetry).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Enqueued {
    Appended,
    Coalesced,
    /// Appended after evicting an older event of the given class
    /// (`EvictedPointer`: a superseded button transition).
    EvictedMotion,
    EvictedPointer,
    EvictedKey,
    /// The incoming motion-only event was dropped: the queue held no older
    /// motion to evict and its button state is already current.
    DroppedIncomingMotion,
}

pub struct InputQueue {
    events: VecDeque<Queued>,
    capacity: usize,
    /// Button state of the last pointer event routed into this queue; the
    /// motion-only test compares against it.
    last_buttons: u8,
}

impl InputQueue {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            events: VecDeque::with_capacity(capacity),
            capacity: capacity.max(1),
            last_buttons: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn pop_front(&mut self) -> Option<InputEventV1> {
        self.events.pop_front().map(|queued| queued.event)
    }

    /// The pre-split ADR-074 rule: drop the oldest event of any class. Kept as
    /// the `atrium-input-queue-legacy` kill-switch and as the KAT reference.
    pub fn enqueue_legacy(&mut self, event: InputEventV1) -> Enqueued {
        let outcome = if self.events.len() < self.capacity {
            Enqueued::Appended
        } else {
            let evicted = self.events.pop_front().map(|q| q.event);
            if evicted.is_some_and(|e| is_pointer(&e)) {
                Enqueued::EvictedPointer
            } else {
                Enqueued::EvictedKey
            }
        };
        self.events.push_back(Queued {
            event,
            motion_only: false,
        });
        outcome
    }

    pub fn enqueue(&mut self, event: InputEventV1) -> Enqueued {
        let buttons_unchanged = event.flags == self.last_buttons;
        let motion_only = is_pointer(&event)
            && !carries_wheel(&event)
            && buttons_unchanged
            && (event.kind == INPUT_EVENT_POINTER || event.code == PHASE_MOVE);
        if is_pointer(&event) {
            self.last_buttons = event.flags;
        }
        if motion_only
            && let Some(tail) = self.events.back_mut()
            && tail.motion_only
            && tail.event.kind == event.kind
            && tail.event.flags == event.flags
        {
            if event.kind == INPUT_EVENT_POINTER {
                tail.event.x = tail.event.x.saturating_add(event.x);
                tail.event.y = tail.event.y.saturating_add(event.y);
            } else {
                // Absolute: the newer position replaces the older.
                tail.event.x = event.x;
                tail.event.y = event.y;
            }
            return Enqueued::Coalesced;
        }
        // Key auto-repeat (`key-autorepeat`): a repeat joins the queue only
        // when the queue has room and holds no unread repeat. A client that
        // stops reading gets at most one, never a backlog of them, and
        // nothing is evicted to make room for one. Counted as coalesced, not
        // as a drop: while the key is held, the next repeat replaces it.
        #[cfg(feature = "key-autorepeat")]
        if super::key_repeat_logic::is_repeat(&event)
            && (self.events.len() >= self.capacity
                || self
                    .events
                    .iter()
                    .any(|q| super::key_repeat_logic::is_repeat(&q.event)))
        {
            return Enqueued::Coalesced;
        }
        // A9-C2 (Chad, #104 RETURN F1): a WM-forwarded wheel (kind 5) is
        // never a button transition, so on a full queue with no motion to
        // evict it is dropped, as a raw wheel is: it never evicts a key or a
        // transition. Kind 5 exists only with `atrium-pointer-wheel`.
        #[cfg(feature = "atrium-pointer-wheel")]
        if event.kind == super::pointer_route_logic::INPUT_EVENT_POINTER_WHEEL
            && self.events.len() >= self.capacity
            && !self.events.iter().any(|q| q.motion_only)
        {
            return Enqueued::DroppedIncomingMotion;
        }
        let outcome = if self.events.len() < self.capacity {
            Enqueued::Appended
        } else if let Some(i) = self.events.iter().position(|q| q.motion_only) {
            self.events.remove(i);
            Enqueued::EvictedMotion
        } else if motion_only || carries_wheel(&event) && buttons_unchanged {
            // Nothing cheaper is queued: drop the incoming motion itself. Its
            // buttons equal the current state, so only a delta (or a wheel
            // step: A9-C2) is lost.
            return Enqueued::DroppedIncomingMotion;
        } else if let Some(i) = self.oldest_superseded_transition(is_pointer(&event)) {
            self.events.remove(i);
            Enqueued::EvictedPointer
        } else {
            // Keys plus at most one unsuperseded pointer event (or, for an
            // incoming button change, nothing but keys): a key overflow. Evict
            // the oldest *key* — never the head blindly, which may be that
            // sole unsuperseded transition (Janus, #38 RETURN). A queue with
            // no key at all can only be a capacity-1 queue holding one
            // pointer event.
            let i = self
                .events
                .iter()
                .position(|q| !is_pointer(&q.event))
                .unwrap_or(0);
            self.events.remove(i);
            Enqueued::EvictedKey
        };
        self.events.push_back(Queued { event, motion_only });
        outcome
    }

    /// The oldest queued button transition that something later supersedes:
    /// a later queued pointer event, or the incoming one (`incoming_pointer`).
    /// Every pointer event carries the full button bitmap, so once a later one
    /// is delivered the client's button state is correct even without the
    /// evicted edge. The newest pointer event is never evicted unless the
    /// incoming event is itself a pointer event, so the final button state
    /// always reaches the client.
    fn oldest_superseded_transition(&self, incoming_pointer: bool) -> Option<usize> {
        let limit = if incoming_pointer {
            self.events.len()
        } else {
            self.events.iter().rposition(|q| is_pointer(&q.event))?
        };
        self.events
            .iter()
            .take(limit)
            .position(|q| is_pointer(&q.event) && !q.motion_only && !carries_wheel(&q.event))
    }
}
