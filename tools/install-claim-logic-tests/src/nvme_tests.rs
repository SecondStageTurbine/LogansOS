// SPDX-License-Identifier: MPL-2.0
//! Muse's R4.1 review, item 4: NVMe namespace changes and resets under the
//! claim. A model controller (one namespace at NSID 1, an admin completion
//! queue, an Asynchronous Event Request it holds until it has an event) and
//! the driver's side of it, which makes the same decisions in the same order
//! as `nvme.rs` (`media_state` → `service_async_events` →
//! `register_verdict`) through the shared pure functions. The QEMU leg
//! `install_claim_nvme_events` runs the real driver against QEMU's NVMe.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use super::claim_logic::{
    ClaimContext, ClaimKind, ClaimTarget, DriverIdentity, Enumerated, Fingerprint, Refusal, Token,
    claim,
};
use super::nvme_event_logic::{
    AEC_NS_ATTR_NOTICES, AerResult, CSTS_NSSRO, CSTS_RDY, LOG_CHANGED_NS_LIST, Regs, aer_action,
    get_log_page_cdw10, namespace_ids, register_verdict,
};

const CAP: u64 = 4 * 1024 * 1024;
const S: u64 = 512;

/// What the probe programs: CC (EN, the queue entry sizes), the admin queue
/// sizes and addresses.
const OURS: Regs = Regs {
    cc: 0x0046_0001,
    csts: CSTS_RDY,
    aqa: 0x0007_0007,
    asq: 0x1234_0000,
    acq: 0x1235_0000,
};

/// A namespace: its bytes and its NGUID || EUI64.
struct Ns {
    bytes: RefCell<Vec<u8>>,
    ids: [u8; 24],
}

fn ns(fill: u8, ids: [u8; 24]) -> Rc<Ns> {
    Rc::new(Ns {
        bytes: RefCell::new(vec![fill; CAP as usize]),
        ids,
    })
}

fn ids(tag: u8) -> [u8; 24] {
    let mut out = [0u8; 24];
    out[0] = tag;
    out[23] = tag;
    out
}

/// The model controller and the driver's state for it.
struct Ctrl {
    // The controller.
    regs: Cell<Regs>,
    attached: RefCell<Rc<Ns>>,
    admin_cq: RefCell<VecDeque<(u16, u32, u32)>>,
    /// The event request it holds, and whether notices are masked until the
    /// Changed Namespace List is read (as QEMU's `changed_nsids`).
    holds_aer: Cell<Option<u16>>,
    masked: Cell<bool>,
    // The driver.
    next_cid: Cell<u16>,
    aer_cid: Cell<Option<u16>>,
    generation: Cell<u64>,
    alive: Cell<bool>,
    logs_read: Cell<u32>,
    /// Writes that reached a namespace.
    writes: Cell<usize>,
}

impl Ctrl {
    /// Probed: notices on, one request posted.
    fn new(first: Rc<Ns>) -> Rc<Self> {
        let c = Rc::new(Self {
            regs: Cell::new(OURS),
            attached: RefCell::new(first),
            admin_cq: RefCell::new(VecDeque::new()),
            holds_aer: Cell::new(None),
            masked: Cell::new(false),
            next_cid: Cell::new(1),
            aer_cid: Cell::new(None),
            generation: Cell::new(0),
            alive: Cell::new(true),
            logs_read: Cell::new(0),
            writes: Cell::new(0),
        });
        c.post_aer();
        c
    }

    fn post_aer(&self) {
        let cid = self.next_cid.get();
        self.next_cid.set(cid + 1);
        self.aer_cid.set(Some(cid));
        self.holds_aer.set(Some(cid));
    }

    /// The controller's side of a namespace change: the attached namespace
    /// is now `to`, and a notice completes the request it holds (unless
    /// masked, or it holds none).
    fn attach(&self, to: Rc<Ns>) {
        *self.attached.borrow_mut() = to;
        if self.masked.get() {
            return;
        }
        if let Some(cid) = self.holds_aer.take() {
            let dw0 = 0x2 | ((LOG_CHANGED_NS_LIST as u32) << 16);
            self.admin_cq.borrow_mut().push_back((cid, 0, dw0));
            self.masked.set(true);
        }
    }

    fn get_log(&self, log: u8) {
        self.logs_read.set(self.logs_read.get() + 1);
        if log == LOG_CHANGED_NS_LIST {
            self.masked.set(false);
        }
    }

    /// The driver's `media_state`.
    fn poll(&self) {
        let mut done = None;
        if let Some((cid, status, dw0)) = self.admin_cq.borrow_mut().pop_front() {
            if self.aer_cid.get() == Some(cid) {
                self.aer_cid.set(None);
                done = Some((status, dw0));
            } else {
                self.dead();
            }
        }
        if let Some((status, dw0)) = done {
            let action = aer_action(status, dw0);
            if !action.rearm {
                self.dead();
            } else {
                self.generation.set(self.generation.get() + 1);
                if let Some(log) = action.read_log {
                    self.get_log(log);
                }
                self.post_aer();
            }
        }
        if self.aer_cid.get().is_none() {
            self.dead();
        }
        if register_verdict(self.regs.get(), OURS).is_some() {
            self.dead();
        }
    }

    fn dead(&self) {
        if self.alive.replace(false) {
            self.generation.set(self.generation.get() + 1);
        }
    }

    // Resets, as NVMe defines them and QEMU emulates them. Each drops the
    // event request the controller held, with no completion.

    /// CC.EN 1 → 0: CC and CSTS clear; AQA, ASQ and ACQ are kept.
    fn controller_reset(&self) {
        let r = self.regs.get();
        self.regs.set(Regs {
            cc: 0,
            csts: 0,
            ..r
        });
        self.holds_aer.set(None);
        self.admin_cq.borrow_mut().clear();
    }

    /// A function-level or conventional (PCIe) reset: every register to its
    /// default.
    fn function_reset(&self) {
        self.regs.set(Regs {
            cc: 0,
            csts: 0,
            aqa: 0,
            asq: 0,
            acq: 0,
        });
        self.holds_aer.set(None);
        self.admin_cq.borrow_mut().clear();
    }

    /// An NVM subsystem reset: the same, and CSTS.NSSRO set after it.
    fn subsystem_reset(&self) {
        self.function_reset();
        let r = self.regs.get();
        self.regs.set(Regs {
            csts: CSTS_NSSRO,
            ..r
        });
    }

    /// Something other than this driver enables the controller again,
    /// writing `regs` (the driver never re-enables after its probe).
    fn enabled_by_another_agent(&self, regs: Regs) {
        self.regs.set(regs);
    }
}

impl ClaimTarget for Rc<Ctrl> {
    fn media_generation(&self) -> u64 {
        self.poll();
        self.generation.get()
    }
    fn alive(&self) -> bool {
        self.poll();
        self.alive.get()
    }
    fn identity(&self) -> Option<DriverIdentity> {
        Some(DriverIdentity {
            model: "QEMU NVMe Ctrl".into(),
            serial: "LOGANSOS-NVME".into(),
            capacity_bytes: CAP,
            sector_size: S as u32,
            namespace_ids: Some(self.attached.borrow().ids),
        })
    }
    fn capacity(&self) -> u64 {
        CAP
    }
    fn write(&self, offset: u64, bytes: &[u8]) -> bool {
        let n = self.attached.borrow().clone();
        n.bytes.borrow_mut()[offset as usize..offset as usize + bytes.len()].copy_from_slice(bytes);
        self.writes.set(self.writes.get() + 1);
        true
    }
    fn read(&self, offset: u64, out: &mut [u8]) -> bool {
        let n = self.attached.borrow().clone();
        out.copy_from_slice(&n.bytes.borrow()[offset as usize..offset as usize + out.len()]);
        true
    }
    fn flush(&self) -> bool {
        true
    }
}

fn enumerate(c: &Rc<Ctrl>) -> Enumerated {
    Enumerated {
        token: Token {
            slot: 0,
            generation: 1,
        },
        identity: c.identity(),
        fingerprint: Fingerprint::read(c).unwrap(),
        disk_guid: None,
    }
}

fn ctx() -> ClaimContext<'static> {
    ClaimContext {
        slot_generation: 1,
        boot_disk_guid: None,
        protected: &[],
        already_claimed: false,
    }
}

fn claimed(c: &Rc<Ctrl>, kind: ClaimKind) -> super::claim_logic::Claim<Rc<Ctrl>> {
    claim(c.clone(), &enumerate(c), kind, &ctx()).unwrap()
}

// === The pure decisions, against hand-laid values ===

/// NGUID is Identify Namespace bytes 104..120, EUI64 120..128.
#[test]
fn namespace_ids_from_identify_namespace() {
    let mut page = vec![0xffu8; 4096];
    for (i, b) in page[104..128].iter_mut().enumerate() {
        *b = 0x10 + i as u8;
    }
    let got = namespace_ids(&page).unwrap();
    assert_eq!(got[0], 0x10, "NGUID byte 0 is page byte 104");
    assert_eq!(got[15], 0x1f, "NGUID byte 15 is page byte 119");
    assert_eq!(got[16], 0x20, "EUI64 byte 0 is page byte 120");
    assert_eq!(got[23], 0x27, "EUI64 byte 7 is page byte 127");
    assert_eq!(namespace_ids(&page[..127]), None);
}

/// Completion Dword 0: type bits 2:0, info 15:8, log page 23:16.
#[test]
fn async_event_completions_decode_and_all_change_the_medium() {
    let notice = AerResult::decode(0x0004_0002);
    assert_eq!((notice.event_type, notice.info, notice.log_page), (2, 0, 4));
    assert!(notice.namespace_attribute_changed());
    let a = aer_action(0, 0x0004_0002);
    assert_eq!(a.reason, "nvme_namespace_attribute_changed");
    assert_eq!(a.read_log, Some(LOG_CHANGED_NS_LIST));
    assert!(a.rearm);
    // A SMART / health event (type 1, log 02h): not enabled, still a change.
    let smart = aer_action(0, 0x0002_0001);
    assert_eq!(smart.reason, "nvme_async_event");
    assert_eq!(smart.read_log, Some(0x02));
    // A firmware-activation notice (type 2, info 01h).
    assert!(!AerResult::decode(0x0003_0102).namespace_attribute_changed());
    // An error completion (aborted): no log, no re-arm; the driver kills the
    // device.
    let err = aer_action(0x7, 0);
    assert_eq!((err.read_log, err.rearm), (None, false));
}

/// Get Log Page CDW10 for log 04h, 4 KiB: NUMDL 1023, RAE clear, LID 04h.
#[test]
fn get_log_page_dword_10() {
    assert_eq!(get_log_page_cdw10(LOG_CHANGED_NS_LIST), 0x03ff_0004);
    assert_eq!(AEC_NS_ATTR_NOTICES, 0x100);
}

/// The registers the driver programmed pass; each reset kind's leftovers do
/// not.
#[test]
fn register_verdicts() {
    assert_eq!(register_verdict(OURS, OURS), None);
    let with = |f: &dyn Fn(&mut Regs)| {
        let mut r = OURS;
        f(&mut r);
        register_verdict(r, OURS)
    };
    assert_eq!(with(&|r| r.csts = u32::MAX), Some("nvme_csts_all_ones"));
    assert_eq!(with(&|r| r.csts |= 0b10), Some("nvme_csts_cfs"));
    assert_eq!(with(&|r| r.csts |= CSTS_NSSRO), Some("nvme_csts_nssro"));
    assert_eq!(with(&|r| r.csts = 0), Some("nvme_csts_not_ready"));
    assert_eq!(with(&|r| r.csts |= 0b0100), Some("nvme_csts_shutdown"));
    assert_eq!(with(&|r| r.cc &= !1), Some("nvme_cc_changed"));
    assert_eq!(
        with(&|r| r.cc |= 1 << 14),
        Some("nvme_cc_changed"),
        "CC.SHN"
    );
    assert_eq!(
        with(&|r| r.asq = 0),
        Some("nvme_admin_queue_registers_changed")
    );
    assert_eq!(
        with(&|r| r.aqa = 0),
        Some("nvme_admin_queue_registers_changed")
    );
}

/// No NGUID and no EUI64: no identity. Destructive refused, fresh allowed.
#[test]
fn all_zero_namespace_ids_refuse_a_destructive_claim() {
    let c = Ctrl::new(ns(0, [0; 24]));
    assert!(!c.identity().unwrap().available());
    assert_eq!(
        claim(c.clone(), &enumerate(&c), ClaimKind::Destructive, &ctx()).err(),
        Some(Refusal::IdentityUnavailable)
    );
    assert!(claim(c.clone(), &enumerate(&c), ClaimKind::Fresh, &ctx()).is_ok());
    let c = Ctrl::new(ns(0, ids(1)));
    assert!(c.identity().unwrap().available());
}

// === The claim against the model controller ===

/// Muse's FAIL sequence, exactly: a destructive claim on namespace 1; the
/// host detaches it and attaches a twin with the same serial and capacity
/// (here even the same NGUID/EUI64 and the same bytes, the worst case); the
/// controller stays ready throughout. The notice stops the claim before the
/// next write is submitted: nothing lands on the twin.
#[test]
fn muse_sequence_stops_the_claim_before_the_twin_is_written() {
    let a = ns(0, ids(1));
    let c = Ctrl::new(a.clone());
    let mut at = claimed(&c, ClaimKind::Destructive);
    at.write(CAP - S, &[0xb1; S as usize]).unwrap();
    assert_eq!(c.writes.get(), 1);

    let twin = ns(0, ids(1));
    c.attach(twin.clone());
    assert_eq!(c.regs.get().csts & CSTS_RDY, CSTS_RDY, "ready throughout");
    assert_eq!(
        at.write(0, &[0xb2; S as usize]),
        Err(Refusal::MediaChangedBefore)
    );
    assert_eq!(c.writes.get(), 1, "nothing was submitted to the twin");
    assert!(twin.bytes.borrow().iter().all(|&b| b == 0));
    assert_eq!(at.write(0, &[0xb2; S as usize]), Err(Refusal::Stopped));
    assert_eq!(at.boundary(), Err(Refusal::Stopped));
    // The driver read the Changed Namespace List and posted the next request.
    assert_eq!(c.logs_read.get(), 1);
    assert!(c.aer_cid.get().is_some() && c.holds_aer.get().is_some());
    assert!(c.alive.get(), "a notice is a change, not a death");
}

/// The notice is re-armed: a second change after the first is reported too,
/// and a claim taken after re-enumerating the first change is stopped by it.
#[test]
fn every_namespace_change_is_reported() {
    let c = Ctrl::new(ns(0, ids(1)));
    c.attach(ns(0, ids(2)));
    assert_eq!(c.media_generation(), 1);
    let mut at = claimed(&c, ClaimKind::Destructive);
    at.write(0, &[1; S as usize]).unwrap();
    c.attach(ns(0, ids(3)));
    assert_eq!(
        at.write(S, &[1; S as usize]),
        Err(Refusal::MediaChangedBefore)
    );
    assert_eq!(c.logs_read.get(), 2);
}

/// A twin namespace with a different NGUID whose notice never arrives (a
/// controller that masks it): the identity catches it, at claim time and at
/// a step boundary.
#[test]
fn twin_namespace_fails_the_identity_at_claim_and_at_the_boundary() {
    let c = Ctrl::new(ns(0, ids(1)));
    let seen = enumerate(&c);
    c.masked.set(true);
    c.attach(ns(0, ids(2)));
    assert_eq!(
        claim(c.clone(), &seen, ClaimKind::Destructive, &ctx()).err(),
        Some(Refusal::IdentityChanged)
    );

    let c = Ctrl::new(ns(0, ids(1)));
    let mut at = claimed(&c, ClaimKind::Destructive);
    at.write(0, &[1; S as usize]).unwrap();
    c.masked.set(true);
    let twin = ns(0, ids(2));
    // The same bytes the claim expects, so only the identity differs.
    twin.bytes.borrow_mut()[..S as usize].copy_from_slice(&[1; S as usize]);
    c.attach(twin);
    at.flush().unwrap();
    assert_eq!(at.boundary(), Err(Refusal::IdentityChanged));
}

/// Each reset kind, between two writes of a claim: the next write is
/// refused before submission.
#[test]
fn each_reset_kind_stops_the_next_write() {
    for (what, reset) in [
        ("controller", Ctrl::controller_reset as fn(&Ctrl)),
        ("function", Ctrl::function_reset),
        ("nvm subsystem", Ctrl::subsystem_reset),
    ] {
        let c = Ctrl::new(ns(0, ids(1)));
        let mut at = claimed(&c, ClaimKind::Destructive);
        at.write(0, &[1; S as usize]).unwrap();
        reset(&c);
        assert_eq!(
            at.write(S, &[1; S as usize]),
            Err(Refusal::DeviceDead),
            "{what}"
        );
        assert_eq!(c.writes.get(), 1, "{what}");
    }
}

/// A reset that completes between two polls: the controller is enabled
/// again before the claim looks. Unless the agent that re-enabled it also
/// wrote back exactly the driver's admin queue registers and cleared
/// NSSRO, a register differs, and the next write is refused. The lost
/// event request is caught too: the driver's request no longer exists, so
/// no notice can arrive (that alone is not seen; the registers are).
#[test]
fn a_reset_completed_between_polls_is_seen_when_any_register_differs() {
    // A function reset, re-enabled with the admin queues somewhere else.
    let c = Ctrl::new(ns(0, ids(1)));
    let mut at = claimed(&c, ClaimKind::Destructive);
    c.function_reset();
    c.enabled_by_another_agent(Regs {
        asq: 0x9000_0000,
        acq: 0x9001_0000,
        ..OURS
    });
    assert_eq!(at.write(0, &[1; S as usize]), Err(Refusal::DeviceDead));
    // An NVM subsystem reset, re-enabled, NSSRO left set.
    let c = Ctrl::new(ns(0, ids(1)));
    let mut at = claimed(&c, ClaimKind::Destructive);
    c.subsystem_reset();
    c.enabled_by_another_agent(Regs {
        csts: CSTS_RDY | CSTS_NSSRO,
        ..OURS
    });
    assert_eq!(at.write(0, &[1; S as usize]), Err(Refusal::DeviceDead));
}

/// The stated limit (THINK §2a item 5, Muse's item 4 case): a controller
/// reset, then an agent other than this driver enables the controller again
/// with exactly the driver's CC, before the next poll. AQA, ASQ and ACQ
/// survive a controller reset, so every register reads as the driver left
/// it, and the dropped event request leaves no trace. Not detected here. On
/// bare metal nothing but this kernel writes the controller's BAR, and the
/// driver never re-enables it after its probe, so this needs a second agent
/// with access to the device (a VM's host, or a hostile one). The I/O queue
/// the claim writes through is deleted by the reset, so its commands fail
/// rather than land: asserted by the QEMU leg, not by this model.
#[test]
fn controller_reset_re_enabled_by_another_agent_is_the_stated_limit() {
    let c = Ctrl::new(ns(0, ids(1)));
    let mut at = claimed(&c, ClaimKind::Destructive);
    c.controller_reset();
    c.enabled_by_another_agent(OURS);
    // The limit: not detected by the registers or the event queue.
    assert_eq!(c.media_generation(), 0);
    assert!(at.write(0, &[1; S as usize]).is_ok());
}
