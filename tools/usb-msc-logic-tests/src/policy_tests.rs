// SPDX-License-Identifier: MPL-2.0
//! OT-100 R2, relay 393 F1–F3: the write path's time bounds, retry policy,
//! latches and diagnostics (`msc_logic`'s `wait_by`, `write_chunk`,
//! `sync_once`, `flush`, `write_probe`), driven with a controlled clock.
//!
//! The clock is a tick counter the mock advances by each step's duration (the
//! kernel's is the 100 Hz `TICKS`). The mock port honours the deadline it is
//! handed the way the kernel's transport does: every wait of a command, its
//! REQUEST SENSE and its recovery stops at that one tick.

use super::msc_logic::*;
use std::collections::VecDeque;

/// Ticks for `ms` milliseconds.
const fn ticks(ms: u64) -> u64 {
    ms / MS_PER_TICK
}

// ---------------------------------------------------------------------------
// `wait_by`: one absolute deadline across a command's waits.
// ---------------------------------------------------------------------------

/// Run `wait_by` for a transfer completing at tick `done_at` on a clock that
/// starts at `*clock` and advances one tick per idle.
fn wait_on(clock: &mut u64, deadline: Deadline, done_at: u64) -> Waited {
    let c = std::cell::Cell::new(*clock);
    let out = wait_by(
        deadline,
        1_000_000,
        || c.get(),
        || {
            if c.get() >= done_at {
                Poll::Done(1, 0)
            } else {
                Poll::Pending
            }
        },
        || c.set(c.get() + 1),
    );
    *clock = c.get();
    out
}

#[test]
fn a_deadline_is_whole_ticks_plus_one() {
    assert_eq!(Deadline::after(0, 10_000).end, 1001);
    assert_eq!(Deadline::after(5, 15).end, 5 + 2 + 1);
    assert!(!Deadline::after(0, 10_000).passed(1000));
    assert!(Deadline::after(0, 10_000).passed(1001));
}

/// Janus's F1 schedule: a WRITE whose CBW, data and CSW each take 9 s. Each
/// fits 10 s alone; under one 10 s deadline the data stage runs out.
#[test]
fn one_deadline_spans_cbw_data_and_csw() {
    let mut clock = 0;
    let by = Deadline::after(clock, WRITE_ATTEMPT_MS);
    assert_eq!(wait_on(&mut clock, by, ticks(9_000)), Waited::Done(1, 0));
    let next = clock + ticks(9_000);
    assert_eq!(wait_on(&mut clock, by, next), Waited::TimedOut);
    assert_eq!(
        clock, by.end,
        "the wait ends at the deadline, not 9 s later"
    );
}

/// The SYNC schedule: CBW after 29 s, CSW 29 s later, under one 30 s deadline.
#[test]
fn one_deadline_spans_a_syncs_cbw_and_csw() {
    let mut clock = 0;
    let by = Deadline::after(clock, SYNC_ATTEMPT_MS);
    assert_eq!(wait_on(&mut clock, by, ticks(29_000)), Waited::Done(1, 0));
    let next = clock + ticks(29_000);
    assert_eq!(wait_on(&mut clock, by, next), Waited::TimedOut);
}

/// A tick that never advances: the poll count ends the wait.
#[test]
fn a_frozen_tick_ends_at_the_backstop() {
    let mut polls = 0;
    let out = wait_by(
        Deadline::after(0, WRITE_ATTEMPT_MS),
        500,
        || 0,
        || {
            polls += 1;
            Poll::Pending
        },
        || {},
    );
    assert_eq!(out, Waited::Backstop);
    assert_eq!(polls, 500);
}

#[test]
fn a_pulled_device_ends_the_wait_at_once() {
    let out = wait_by(
        Deadline::after(0, WRITE_ATTEMPT_MS),
        500,
        || 0,
        || Poll::Gone,
        || {},
    );
    assert_eq!(out, Waited::Gone);
}

// ---------------------------------------------------------------------------
// The policy over a mock port.
// ---------------------------------------------------------------------------

/// One scripted command: how long it takes, its outcome, the bytes of an
/// In stage, and the stage it stood at if the deadline cuts it short.
struct Step {
    ticks: u64,
    outcome: Outcome,
    data_in: Vec<u8>,
    cut_at: ResetReason,
}

fn step(ticks: u64, outcome: Outcome) -> Step {
    Step {
        ticks,
        outcome,
        data_in: Vec::new(),
        cut_at: ResetReason::Data,
    }
}

fn failed(ticks: u64) -> Step {
    step(ticks, Outcome::Failed { len: 0 })
}

fn passed(ticks: u64, len: usize) -> Step {
    step(ticks, Outcome::Passed { len })
}

/// A sense reply: its duration and its bytes (fixed format, decoded by
/// `decode_sense` like the kernel's).
fn sense_reply(ticks: u64, key: u8, asc: u8, ascq: u8) -> (u64, Option<Sense>) {
    let mut bytes = [0u8; 18];
    bytes[0] = 0x70;
    bytes[2] = key;
    bytes[7] = 0x0a;
    bytes[12] = asc;
    bytes[13] = ascq;
    (ticks, decode_sense(&bytes))
}

#[derive(Default)]
struct Mock {
    now: u64,
    gone: bool,
    steps: VecDeque<Step>,
    senses: VecDeque<(u64, Option<Sense>)>,
    /// Every command sent: opcode and its deadline.
    sent: Vec<(u8, Deadline)>,
    /// Every REQUEST SENSE: its deadline.
    sensed: Vec<Deadline>,
    pauses: Vec<u64>,
    events: Vec<WriteEvent>,
}

impl Mock {
    fn new(steps: Vec<Step>, senses: Vec<(u64, Option<Sense>)>) -> Self {
        Mock {
            steps: steps.into(),
            senses: senses.into(),
            ..Mock::default()
        }
    }

    fn opcodes(&self) -> Vec<u8> {
        self.sent.iter().map(|(op, _)| *op).collect()
    }
}

impl WritePort for Mock {
    fn now(&mut self) -> u64 {
        self.now
    }

    fn gone(&self) -> bool {
        self.gone
    }

    fn command(&mut self, cdb: &[u8], data: Data<'_>, by: Deadline) -> Outcome {
        self.sent.push((cdb[0], by));
        let s = self.steps.pop_front().expect("an unscripted command");
        if self.now + s.ticks >= by.end {
            // Every wait, recovery included, stops at `by`.
            self.now = by.end;
            return Outcome::Reset {
                reason: s.cut_at,
                recovered: false,
            };
        }
        self.now += s.ticks;
        if let Data::In(buf) = data {
            buf[..s.data_in.len()].copy_from_slice(&s.data_in);
        }
        s.outcome
    }

    fn sense(&mut self, by: Deadline) -> Option<Sense> {
        self.sensed.push(by);
        let (t, sense) = self.senses.pop_front().expect("an unscripted sense");
        if self.now + t >= by.end {
            self.now = by.end;
            return None;
        }
        self.now += t;
        sense
    }

    fn pause(&mut self, ms: u64) {
        self.pauses.push(ms);
        self.now += ticks(ms);
    }

    fn event(&mut self, event: WriteEvent) {
        self.events.push(event);
    }
}

const BYTES: [u8; 1024] = [0x5a; 1024];
const LBA: u32 = 63;
const BLOCKS: u16 = 2;

fn chunk(port: &mut Mock, latches: &WriteLatches) -> ChunkResult {
    write_chunk(port, latches, LBA, BLOCKS, &BYTES)
}

/// A slow write that finishes inside its one deadline succeeds, once.
#[test]
fn a_slow_write_inside_its_deadline_succeeds() {
    let mut port = Mock::new(vec![passed(ticks(9_500), BYTES.len())], vec![]);
    let latches = WriteLatches::new();
    assert_eq!(chunk(&mut port, &latches), ChunkResult::Written);
    assert_eq!(port.opcodes(), vec![OP_WRITE_10]);
    assert!(port.events.is_empty());
}

/// F1: a failed WRITE's REQUEST SENSE runs under the WRITE's own deadline,
/// not a fresh one; when it can't finish in time, the chunk ends with a
/// deadline line naming REQUEST SENSE.
#[test]
fn the_sense_after_a_failed_write_shares_its_deadline() {
    let mut port = Mock::new(
        vec![failed(ticks(9_000))],
        vec![sense_reply(ticks(2_000), 0x6, 0x28, 0)],
    );
    let latches = WriteLatches::new();
    assert_eq!(chunk(&mut port, &latches), ChunkResult::Io);
    assert_eq!(port.sensed, vec![port.sent[0].1]);
    assert_eq!(
        port.opcodes(),
        vec![OP_WRITE_10],
        "no retry after the deadline"
    );
    assert_eq!(
        port.events,
        vec![WriteEvent::Deadline {
            op: Op::Write,
            lba: LBA,
            blocks: BLOCKS,
            ms: WRITE_ATTEMPT_MS,
            stage: None,
            elapsed: port.sent[0].1.end,
        }]
    );
}

/// F1 + F3: a data stage (or its recovery) still running at the deadline
/// ends the chunk with a deadline line naming the stage and the chunk's
/// block count, not a generic reset line, and is not retried.
#[test]
fn a_command_cut_off_by_its_deadline_is_final_and_named() {
    let mut port = Mock::new(
        vec![step(ticks(60_000), Outcome::Passed { len: 0 })],
        vec![],
    );
    let latches = WriteLatches::new();
    assert_eq!(
        write_chunk(&mut port, &latches, LBA, 128, &[0u8; 65536]),
        ChunkResult::Io
    );
    assert_eq!(port.opcodes(), vec![OP_WRITE_10]);
    assert_eq!(port.now, port.sent[0].1.end, "it ended at the deadline");
    assert_eq!(
        port.events,
        vec![WriteEvent::Deadline {
            op: Op::Write,
            lba: LBA,
            blocks: 128,
            ms: WRITE_ATTEMPT_MS,
            stage: Some(ResetReason::Data),
            elapsed: port.sent[0].1.end,
        }]
    );
}

/// F3: UNIT ATTENTION, then a successful retry: written, two WRITEs, and the
/// `sense=6/…` discriminator is reported. Each attempt has its own deadline.
#[test]
fn a_unit_attention_retry_is_reported() {
    let mut port = Mock::new(
        vec![failed(ticks(100)), passed(ticks(100), BYTES.len())],
        vec![sense_reply(ticks(10), 0x6, 0x28, 0x00)],
    );
    let latches = WriteLatches::new();
    assert_eq!(chunk(&mut port, &latches), ChunkResult::Written);
    assert_eq!(port.opcodes(), vec![OP_WRITE_10, OP_WRITE_10]);
    assert_eq!(
        port.events,
        vec![WriteEvent::UnitAttention {
            op: Op::Write,
            lba: LBA,
            blocks: BLOCKS,
            sense: Sense {
                key: 0x6,
                asc: 0x28,
                ascq: 0x00
            },
        }]
    );
    assert!(port.sent[1].1.end > port.sent[0].1.end);
}

/// NOT READY forever: one try plus NOT_READY_TRIES retries, a pause before
/// each retry, then an I/O error.
#[test]
fn not_ready_is_retried_a_bounded_number_of_times() {
    let n = 1 + NOT_READY_TRIES as usize;
    let mut port = Mock::new(
        (0..n).map(|_| failed(ticks(100))).collect(),
        (0..n)
            .map(|_| sense_reply(ticks(10), 0x2, 0x04, 0x01))
            .collect(),
    );
    let latches = WriteLatches::new();
    assert_eq!(chunk(&mut port, &latches), ChunkResult::Io);
    assert_eq!(port.sent.len(), n);
    assert_eq!(
        port.pauses,
        vec![NOT_READY_PAUSE_MS; NOT_READY_TRIES as usize]
    );
    assert!(matches!(
        port.events.last(),
        Some(WriteEvent::Failed { .. })
    ));
}

/// The worst sequence the policy allows: a UNIT ATTENTION, every NOT READY
/// retry, one recovered reset, then another reset. Exactly
/// WRITE_MAX_ATTEMPTS WRITEs, within `write_chunk_bound_ms`.
#[test]
fn the_chunk_is_bounded_in_attempts_and_time() {
    let attempt = ticks(WRITE_ATTEMPT_MS) - 1;
    let mut steps = vec![failed(attempt / 2)];
    let mut senses = vec![sense_reply(attempt / 2 - 1, 0x6, 0x29, 0)];
    for _ in 0..NOT_READY_TRIES {
        steps.push(failed(attempt / 2));
        senses.push(sense_reply(attempt / 2 - 1, 0x2, 0x04, 0x01));
    }
    let reset = |t| {
        step(
            t,
            Outcome::Reset {
                reason: ResetReason::Csw,
                recovered: true,
            },
        )
    };
    steps.push(reset(attempt - 1));
    steps.push(reset(attempt - 1));
    let mut port = Mock::new(steps, senses);
    let latches = WriteLatches::new();
    assert_eq!(chunk(&mut port, &latches), ChunkResult::Io);
    assert_eq!(port.sent.len(), WRITE_MAX_ATTEMPTS as usize);
    assert!(port.now * MS_PER_TICK <= write_chunk_bound_ms());
}

/// A WRITE refused with DATA PROTECT latches write-protect.
#[test]
fn data_protect_on_a_write_latches_write_protect() {
    let mut port = Mock::new(vec![failed(10)], vec![sense_reply(1, 0x7, 0x27, 0)]);
    let latches = WriteLatches::new();
    latches.note_mode_sense(false);
    assert_eq!(chunk(&mut port, &latches), ChunkResult::WriteProtected);
    assert_eq!(latches.write_protected(), Some(true));
}

/// F2, Janus's input: SYNC fails with fixed-format sense
/// `70 00 07 00 00 00 00 0a 00 00 00 00 27 00 00 00 00 00` (DATA PROTECT,
/// WRITE PROTECTED). Write-protect latches, the flush ends at once with
/// WriteProtected (no retries), and the latch survives a later MODE SENSE
/// that says "not protected".
#[test]
fn data_protect_on_sync_latches_write_protect() {
    let sense = decode_sense(&[
        0x70, 0x00, 0x07, 0x00, 0x00, 0x00, 0x00, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x27, 0x00, 0x00,
        0x00, 0x00, 0x00,
    ]);
    let mut port = Mock::new(vec![failed(10)], vec![(1, sense)]);
    let latches = WriteLatches::new();
    latches.note_mode_sense(false);
    latches.set_sync_ok(true);
    assert_eq!(latches.write_protected(), Some(false));
    assert_eq!(flush(&mut port, &latches), FlushResult::WriteProtected);
    assert_eq!(latches.write_protected(), Some(true));
    assert_eq!(port.opcodes(), vec![OP_SYNCHRONIZE_CACHE_10]);
    assert_eq!(
        port.events,
        vec![WriteEvent::DataProtect {
            op: Op::Sync,
            lba: 0
        }]
    );
    latches.note_mode_sense(false);
    assert_eq!(latches.write_protected(), Some(true), "the latch holds");
}

/// ILLEGAL REQUEST on SYNC turns flushing off for good.
#[test]
fn illegal_request_on_sync_turns_flush_off() {
    let mut port = Mock::new(vec![failed(10)], vec![sense_reply(1, 0x5, 0x20, 0)]);
    let latches = WriteLatches::new();
    latches.set_sync_ok(true);
    assert_eq!(flush(&mut port, &latches), FlushResult::Unsupported);
    assert!(!latches.sync_ok());
    assert_eq!(port.events, vec![WriteEvent::SyncUnsupported]);
    assert_eq!(flush(&mut port, &latches), FlushResult::Unsupported);
    assert_eq!(port.sent.len(), 1, "nothing sent once it's off");
}

/// A flush with flushing never shown to work sends nothing.
#[test]
fn an_unprobed_flush_sends_nothing() {
    let mut port = Mock::new(vec![], vec![]);
    let latches = WriteLatches::new();
    assert_eq!(flush(&mut port, &latches), FlushResult::Unsupported);
    assert!(port.sent.is_empty());
}

/// A SYNC's REQUEST SENSE shares the SYNC's deadline too.
#[test]
fn the_sense_after_a_failed_sync_shares_its_deadline() {
    let mut port = Mock::new(
        vec![failed(ticks(29_000))],
        vec![sense_reply(ticks(2_000), 0x3, 0, 0)],
    );
    let latches = WriteLatches::new();
    assert_eq!(sync_once(&mut port, &latches), SyncResult::Failed);
    assert_eq!(port.sensed, vec![port.sent[0].1]);
    assert!(matches!(
        port.events[..],
        [WriteEvent::Deadline {
            op: Op::Sync,
            stage: None,
            ..
        }]
    ));
}

/// The probe: MODE SENSE, TEST UNIT READY, SYNC, in that order; a DATA
/// PROTECT on its SYNC latches write-protect over MODE SENSE's "no".
#[test]
fn the_probe_latches_data_protect_from_its_sync() {
    let mut header = passed(10, 4);
    header.data_in = vec![0x03, 0x00, 0x00, 0x00];
    let mut port = Mock::new(
        vec![header, passed(10, 0), failed(10)],
        vec![sense_reply(1, 0x7, 0x27, 0)],
    );
    let latches = WriteLatches::new();
    let probe = write_probe(&mut port, &latches);
    assert_eq!(
        port.opcodes(),
        vec![OP_MODE_SENSE_6, 0x00, OP_SYNCHRONIZE_CACHE_10]
    );
    assert_eq!(probe.write_protected, Some(true));
    assert_eq!(probe.sync, SyncResult::WriteProtected);
    assert!(!latches.sync_ok());
}

/// The probe on a healthy stick: not protected, flushing on.
#[test]
fn the_probe_on_a_healthy_stick() {
    let mut header = passed(10, 4);
    header.data_in = vec![0x03, 0x00, 0x00, 0x00];
    let mut port = Mock::new(vec![header, passed(10, 0), passed(10, 0)], vec![]);
    let latches = WriteLatches::new();
    let probe = write_probe(&mut port, &latches);
    assert_eq!(probe.write_protected, Some(false));
    assert_eq!(probe.sync, SyncResult::Synced);
    assert!(latches.sync_ok());
}

/// A stick pulled before a chunk: nothing is sent.
#[test]
fn a_pulled_stick_is_not_written() {
    let mut port = Mock::new(vec![], vec![]);
    port.gone = true;
    let latches = WriteLatches::new();
    assert_eq!(chunk(&mut port, &latches), ChunkResult::NoDevice);
    assert!(port.sent.is_empty());
}
