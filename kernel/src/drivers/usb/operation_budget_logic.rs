// SPDX-License-Identifier: MPL-2.0
//! Pure accounting core for one operation-wide USB control-plane budget.
//!
//! A per-wait timeout is not a bound on a composed operation: one control
//! transfer may wait for several events, one vendor request may retry several
//! transfers, and one register poll may issue several vendor requests.  The
//! caller therefore charges every layer against one non-replenishing ledger.
//! `xhci` uses [`charge_step`] with atomics; host tests use [`BudgetModel`]
//! directly, so the oracle and the device path share the same decision core.

#![allow(dead_code)] // host-oracle'd pure core; xHCI is the production consumer

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChargeKind {
    WaitCall,
    WaitIteration,
    VendorAttempt,
    VendorRetryGap,
    Observation,
    PollGap,
    BulkAttempt,
    BulkPoll,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BudgetCounters {
    pub wait_calls: u64,
    pub wait_iterations: u64,
    pub vendor_attempts: u64,
    pub vendor_retry_gaps: u64,
    pub vendor_request_exhaustions: u64,
    pub observations: u64,
    pub poll_gaps: u64,
    pub bulk_attempts: u64,
    pub bulk_polls: u64,
    pub late_events: u64,
    pub no_event_exhaustions: u64,
    pub desync_exhaustions: u64,
}

impl BudgetCounters {
    pub const fn charged_total(self) -> u64 {
        self.wait_calls
            + self.wait_iterations
            + self.vendor_attempts
            + self.vendor_retry_gaps
            + self.observations
            + self.poll_gaps
            + self.bulk_attempts
            + self.bulk_polls
    }

    pub fn note_charge(&mut self, kind: ChargeKind) {
        match kind {
            ChargeKind::WaitCall => self.wait_calls += 1,
            ChargeKind::WaitIteration => self.wait_iterations += 1,
            ChargeKind::VendorAttempt => self.vendor_attempts += 1,
            ChargeKind::VendorRetryGap => self.vendor_retry_gaps += 1,
            ChargeKind::Observation => self.observations += 1,
            ChargeKind::PollGap => self.poll_gaps += 1,
            ChargeKind::BulkAttempt => self.bulk_attempts += 1,
            ChargeKind::BulkPoll => self.bulk_polls += 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChargeStep {
    Charged { remaining: u64 },
    Exhausted,
}

/// Consume one structural unit without ever replenishing the caller's budget.
pub const fn charge_step(remaining: u64) -> ChargeStep {
    if remaining == 0 {
        ChargeStep::Exhausted
    } else {
        ChargeStep::Charged {
            remaining: remaining - 1,
        }
    }
}

/// Device-free model of the exact accounting decision used by xHCI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BudgetModel {
    limit: u64,
    remaining: u64,
    counters: BudgetCounters,
    exhausted: bool,
}

impl BudgetModel {
    pub const fn new(limit: u64) -> Self {
        Self {
            limit,
            remaining: limit,
            counters: BudgetCounters {
                wait_calls: 0,
                wait_iterations: 0,
                vendor_attempts: 0,
                vendor_retry_gaps: 0,
                vendor_request_exhaustions: 0,
                observations: 0,
                poll_gaps: 0,
                bulk_attempts: 0,
                bulk_polls: 0,
                late_events: 0,
                no_event_exhaustions: 0,
                desync_exhaustions: 0,
            },
            exhausted: false,
        }
    }

    pub fn charge(&mut self, kind: ChargeKind) -> bool {
        match charge_step(self.remaining) {
            ChargeStep::Charged { remaining } => {
                self.remaining = remaining;
                self.counters.note_charge(kind);
                true
            }
            ChargeStep::Exhausted => {
                self.exhausted = true;
                false
            }
        }
    }

    pub fn note_late_event(&mut self) {
        self.counters.late_events += 1;
    }

    pub fn note_vendor_request_exhaustion(&mut self) {
        self.counters.vendor_request_exhaustions += 1;
    }

    pub fn note_no_event_exhaustion(&mut self) {
        self.counters.no_event_exhaustions += 1;
    }

    pub fn note_desync_exhaustion(&mut self) {
        self.counters.desync_exhaustions += 1;
    }

    pub const fn limit(&self) -> u64 {
        self.limit
    }

    pub const fn remaining(&self) -> u64 {
        self.remaining
    }

    pub const fn counters(&self) -> BudgetCounters {
        self.counters
    }

    pub const fn exhausted(&self) -> bool {
        self.exhausted
    }
}
