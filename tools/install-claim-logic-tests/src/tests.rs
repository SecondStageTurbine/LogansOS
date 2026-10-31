// SPDX-License-Identifier: MPL-2.0
//! The §2a test table (`docs/THINK-2026-10-27-ot87-installer.md`), on a model.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::claim_logic::{
    ClaimContext, ClaimKind, ClaimTarget, DriverIdentity, Enumerated, Fingerprint, Refusal, Token,
    WINDOW_BYTES, Windows, ascii_field, ata_capacity, ata_serial_model, claim, nvme_serial_model,
    virtio_serial,
};

const CAP: u64 = 8 * 1024 * 1024; // 8 MiB: two 1 MiB windows and an interior
const S: u64 = 512;

/// A medium: its bytes and what it reports.
struct Medium {
    bytes: RefCell<Vec<u8>>,
    identity: Option<DriverIdentity>,
}

fn medium(serial: &str, fill: u8) -> Rc<Medium> {
    Rc::new(Medium {
        bytes: RefCell::new(vec![fill; CAP as usize]),
        identity: Some(DriverIdentity {
            model: "QEMU HARDDISK".into(),
            serial: serial.into(),
            capacity_bytes: CAP,
            sector_size: S as u32,
            namespace_ids: None,
        }),
    })
}

/// A device object: the medium behind it can be swapped (reported or not),
/// and it can bump its media generation during a write.
struct Device {
    medium: RefCell<Rc<Medium>>,
    media_generation: Cell<u64>,
    alive: Cell<bool>,
    bump_during_next_write: Cell<bool>,
    /// Writes that reached a medium.
    writes: Cell<usize>,
}

impl Device {
    fn new(m: Rc<Medium>) -> Rc<Self> {
        Rc::new(Self {
            medium: RefCell::new(m),
            media_generation: Cell::new(0),
            alive: Cell::new(true),
            bump_during_next_write: Cell::new(false),
            writes: Cell::new(0),
        })
    }

    /// The medium changes behind the same device object. `reported`: the
    /// driver bumps its media generation (a link reset, a removal).
    fn swap(&self, m: Rc<Medium>, reported: bool) {
        *self.medium.borrow_mut() = m;
        if reported {
            self.media_generation.set(self.media_generation.get() + 1);
        }
    }
}

impl ClaimTarget for Rc<Device> {
    fn media_generation(&self) -> u64 {
        self.media_generation.get()
    }
    fn alive(&self) -> bool {
        self.alive.get()
    }
    fn identity(&self) -> Option<DriverIdentity> {
        self.medium.borrow().identity.clone()
    }
    fn capacity(&self) -> u64 {
        CAP
    }
    fn write(&self, offset: u64, bytes: &[u8]) -> bool {
        if self.bump_during_next_write.replace(false) {
            // The medium is replaced while this request is in flight: the
            // write lands on the new medium and the driver reports it.
            let other = medium("B-0001", 0);
            self.swap(other, true);
        }
        let m = self.medium.borrow().clone();
        m.bytes.borrow_mut()[offset as usize..offset as usize + bytes.len()].copy_from_slice(bytes);
        self.writes.set(self.writes.get() + 1);
        true
    }
    fn flush(&self) -> bool {
        true
    }
    fn read(&self, offset: u64, out: &mut [u8]) -> bool {
        let m = self.medium.borrow().clone();
        out.copy_from_slice(&m.bytes.borrow()[offset as usize..offset as usize + out.len()]);
        true
    }
}

/// The model registry: slots with generations (§2a item 1).
struct Registry {
    slots: Vec<(u64, Rc<Device>)>,
}

impl Registry {
    fn enumerate(&self, slot: usize) -> Enumerated {
        let (generation, device) = &self.slots[slot];
        Enumerated {
            token: Token {
                slot,
                generation: *generation,
            },
            identity: device.identity(),
            fingerprint: Fingerprint::read(device).unwrap(),
            disk_guid: None,
        }
    }

    /// A removal, a reinsertion or a re-registration: the slot's generation
    /// moves on.
    fn replace(&mut self, slot: usize, device: Rc<Device>) {
        let generation = self.slots[slot].0 + 1;
        self.slots[slot] = (generation, device);
    }

    fn ctx(&self, slot: usize) -> ClaimContext<'static> {
        ClaimContext {
            slot_generation: self.slots[slot].0,
            boot_disk_guid: None,
            protected: &[],
            already_claimed: false,
        }
    }
}

fn one(device: &Rc<Device>) -> Registry {
    Registry {
        slots: vec![(7, device.clone())],
    }
}

/// W1 and W2 of §4.7, in miniature: the backup header in the last sector,
/// then the primary table in sectors 0..34.
fn w1(at: &mut super::claim_logic::Claim<Rc<Device>>) {
    at.write(CAP - S, &[0xb1; S as usize]).unwrap();
}

fn w2(at: &mut super::claim_logic::Claim<Rc<Device>>) {
    at.write(0, &[0xb2; 34 * S as usize]).unwrap();
}

// === Identity parsers, against hand-laid bytes ===

#[test]
fn nvme_serial_and_model_from_identify_controller() {
    let mut id = vec![0u8; 4096];
    id[4..24].copy_from_slice(b"S5GXNX0T123456A     "); // SN, space-padded
    id[24..64].copy_from_slice(b"Samsung SSD 980 PRO 1TB                 ");
    let (sn, mn) = nvme_serial_model(&id);
    assert_eq!(sn, "S5GXNX0T123456A");
    assert_eq!(mn, "Samsung SSD 980 PRO 1TB");
    assert_eq!(nvme_serial_model(&id[..32]), (String::new(), String::new()));
}

#[test]
fn ata_serial_and_model_are_byte_swapped_per_word() {
    let mut id = vec![0u8; 512];
    // "QM00001" as ATA stores it in words 10..19: each pair swapped, padded
    // with spaces (QEMU's IDE serial).
    let serial = b"QM00001             ";
    let model = b"QEMU HARDDISK                           ";
    for (i, pair) in serial.chunks(2).enumerate() {
        id[20 + 2 * i] = pair[1];
        id[20 + 2 * i + 1] = pair[0];
    }
    for (i, pair) in model.chunks(2).enumerate() {
        id[54 + 2 * i] = pair[1];
        id[54 + 2 * i + 1] = pair[0];
    }
    // Word 10's two bytes, laid out by hand: 'M' then 'Q'.
    assert_eq!(&id[20..22], b"MQ");
    assert_eq!(
        ata_serial_model(&id),
        ("QM00001".into(), "QEMU HARDDISK".into())
    );
}

/// ATA capacity, bytes laid out by hand: 48-bit words 100–103 win over the
/// 28-bit words 60–61, and word 106 gives a 4096-byte logical sector only
/// when it is valid and bit 12 is set.
#[test]
fn ata_capacity_from_identify_words() {
    let mut id = vec![0u8; 512];
    // Words 60–61 = 0x0fff_ffff (the 28-bit limit), words 100–103 =
    // 0x0000_0001_2a19_c000 sectors: low word first, each little-endian.
    id[120..124].copy_from_slice(&[0xff, 0xff, 0xff, 0x0f]);
    id[200..208].copy_from_slice(&[0x00, 0xc0, 0x19, 0x2a, 0x01, 0x00, 0x00, 0x00]);
    assert_eq!(ata_capacity(&id), (0x1_2a19_c000, 512));
    // No 48-bit count: the 28-bit one.
    id[200..208].fill(0);
    assert_eq!(ata_capacity(&id), (0x0fff_ffff, 512));
    // Word 106 = 0x5000 (valid, bit 12), words 117–118 = 2048 words.
    id[212..214].copy_from_slice(&[0x00, 0x50]);
    id[234..238].copy_from_slice(&[0x00, 0x08, 0x00, 0x00]);
    assert_eq!(ata_capacity(&id), (0x0fff_ffff, 4096));
    // Word 106 = 0x1000: bit 12 but not valid (bits 15:14 = 00b): 512.
    id[212..214].copy_from_slice(&[0x00, 0x10]);
    assert_eq!(ata_capacity(&id), (0x0fff_ffff, 512));
    assert_eq!(ata_capacity(&id[..100]), (0, 0));
}

#[test]
fn virtio_serial_is_20_nul_padded_bytes() {
    let mut id = [0u8; 20];
    id[..13].copy_from_slice(b"LOGANSOS-TEST");
    assert_eq!(virtio_serial(&id), "LOGANSOS-TEST");
    // A 20-character serial has no NUL.
    assert_eq!(
        virtio_serial(b"ABCDEFGHIJKLMNOPQRST"),
        "ABCDEFGHIJKLMNOPQRST"
    );
    // No `serial=`: all zero, no identity.
    assert_eq!(virtio_serial(&[0u8; 20]), "");
}

#[test]
fn blank_or_unprintable_serials_are_no_identity() {
    let id = |serial: &str| DriverIdentity {
        model: "m".into(),
        serial: serial.into(),
        capacity_bytes: CAP,
        sector_size: 512,
        namespace_ids: None,
    };
    assert!(id("QM00001").available());
    assert!(!id("").available());
    assert_eq!(ascii_field(b"    \0\0  "), "");
    assert_eq!(ascii_field(b"AB\x01CD"), "", "a control byte inside");
    assert_eq!(ascii_field(b"AB\xffCD"), "", "a non-ASCII byte inside");
    assert_eq!(ascii_field(b"  A B  "), "A B");
}

// === The §2a test table ===

/// Reorder: the slot behind a token now holds another device.
#[test]
fn reorder_refuses_the_stale_token() {
    let a = Device::new(medium("A-0001", 0));
    let mut reg = one(&a);
    let seen = reg.enumerate(0);
    let b = Device::new(medium("B-0001", 0));
    reg.replace(0, b.clone());
    let r = claim(b.clone(), &seen, ClaimKind::Fresh, &reg.ctx(0));
    assert_eq!(r.err(), Some(Refusal::StaleToken));
    assert_eq!(b.writes.get() + a.writes.get(), 0);
}

/// Removal and reinsertion of the same disk: a new generation, the old token
/// refused; a fresh enumeration claims it.
#[test]
fn removal_and_reinsert_needs_a_new_enumeration() {
    let m = medium("A-0001", 0);
    let a = Device::new(m.clone());
    let mut reg = one(&a);
    let seen = reg.enumerate(0);
    a.alive.set(false);
    assert_eq!(
        claim(a.clone(), &seen, ClaimKind::Fresh, &reg.ctx(0)).err(),
        Some(Refusal::DeviceDead)
    );
    let again = Device::new(m);
    reg.replace(0, again.clone());
    assert_eq!(
        claim(again.clone(), &seen, ClaimKind::Fresh, &reg.ctx(0)).err(),
        Some(Refusal::StaleToken)
    );
    let fresh = reg.enumerate(0);
    assert!(claim(again, &fresh, ClaimKind::Fresh, &reg.ctx(0)).is_ok());
}

/// Identity unavailable (a virtio disk without `serial=`): destructive
/// refused with the reason, fresh allowed.
#[test]
fn unavailable_identity_fails_closed_for_destructive_only() {
    let m = Rc::new(Medium {
        bytes: RefCell::new(vec![0; CAP as usize]),
        identity: Some(DriverIdentity {
            model: "".into(),
            serial: "".into(),
            capacity_bytes: CAP,
            sector_size: 512,
            namespace_ids: None,
        }),
    });
    let a = Device::new(m);
    let reg = one(&a);
    let seen = reg.enumerate(0);
    assert_eq!(
        claim(a.clone(), &seen, ClaimKind::Destructive, &reg.ctx(0)).err(),
        Some(Refusal::IdentityUnavailable)
    );
    assert!(claim(a.clone(), &seen, ClaimKind::Fresh, &reg.ctx(0)).is_ok());
    // No identity reported at all is the same.
    let none = Device::new(Rc::new(Medium {
        bytes: RefCell::new(vec![0; CAP as usize]),
        identity: None,
    }));
    let reg = one(&none);
    let seen = reg.enumerate(0);
    assert_eq!(
        claim(none.clone(), &seen, ClaimKind::Destructive, &reg.ctx(0)).err(),
        Some(Refusal::IdentityUnavailable)
    );
    assert!(claim(none, &seen, ClaimKind::Fresh, &reg.ctx(0)).is_ok());
}

/// Replacement with the same tuple before the first write: the windows
/// differ, so the claim is refused before any write.
#[test]
fn same_tuple_replacement_refused_before_the_first_write() {
    let a = Device::new(medium("A-0001", 0));
    let reg = one(&a);
    let seen = reg.enumerate(0);
    let b = medium("A-0001", 0);
    b.bytes.borrow_mut()[100] = 0x55; // different contents, same identity
    a.swap(b, false);
    assert_eq!(
        claim(a.clone(), &seen, ClaimKind::Fresh, &reg.ctx(0)).err(),
        Some(Refusal::ChangedSinceEnumeration)
    );
    assert_eq!(a.writes.get(), 0);
}

/// Replacement after the first write, between steps, unreported: the expected
/// windows include our write, the replacement lacks it.
#[test]
fn replacement_after_the_first_write_fails_the_boundary() {
    let a = Device::new(medium("A-0001", 0));
    let reg = one(&a);
    let mut c = claim(a.clone(), &reg.enumerate(0), ClaimKind::Fresh, &reg.ctx(0)).unwrap();
    w1(&mut c);
    c.boundary().unwrap();
    a.swap(medium("A-0001", 0), false);
    assert_eq!(c.boundary(), Err(Refusal::FingerprintMismatch));
    assert_eq!(c.write(0, &[1; 512]), Err(Refusal::Stopped));
    assert_eq!(a.writes.get(), 1, "nothing written after the stop");
}

/// Replacement between validation and submission, reported: the device
/// bumps its media generation during the request. The post-completion check
/// stops the claim after that one request.
#[test]
fn reported_swap_mid_request_stops_after_that_request() {
    let a = Device::new(medium("A-0001", 0));
    let reg = one(&a);
    let mut c = claim(a.clone(), &reg.enumerate(0), ClaimKind::Fresh, &reg.ctx(0)).unwrap();
    w1(&mut c);
    a.bump_during_next_write.set(true);
    assert_eq!(c.write(0, &[2; 512]), Err(Refusal::MediaChangedAfter));
    assert_eq!(c.stopped(), Some(Refusal::MediaChangedAfter));
    for _ in 0..3 {
        assert_eq!(c.write(512, &[3; 512]), Err(Refusal::Stopped));
    }
    assert_eq!(c.boundary(), Err(Refusal::Stopped));
    assert_eq!(
        a.writes.get(),
        2,
        "the in-flight request is the exposure, no more"
    );
}

/// A reported swap between two requests: the next pre-check refuses, and
/// nothing is submitted.
#[test]
fn reported_swap_between_requests_submits_nothing() {
    let a = Device::new(medium("A-0001", 0));
    let reg = one(&a);
    let mut c = claim(a.clone(), &reg.enumerate(0), ClaimKind::Fresh, &reg.ctx(0)).unwrap();
    w1(&mut c);
    a.swap(medium("A-0001", 0), true);
    assert_eq!(c.write(0, &[2; 512]), Err(Refusal::MediaChangedBefore));
    assert_eq!(a.writes.get(), 1);
}

/// Unreported replacement mid-step with a different identity, or the same
/// identity and different sampled bytes: caught at the step's boundary.
#[test]
fn unreported_differing_replacement_is_caught_at_the_boundary() {
    let a = Device::new(medium("A-0001", 0));
    let reg = one(&a);
    let mut c = claim(a.clone(), &reg.enumerate(0), ClaimKind::Fresh, &reg.ctx(0)).unwrap();
    w1(&mut c);
    a.swap(medium("B-0002", 0), false);
    assert_eq!(c.boundary(), Err(Refusal::IdentityChanged));

    let a = Device::new(medium("A-0001", 0));
    let reg = one(&a);
    let mut c = claim(a.clone(), &reg.enumerate(0), ClaimKind::Fresh, &reg.ctx(0)).unwrap();
    w1(&mut c);
    let b = medium("A-0001", 0);
    b.bytes.borrow_mut()[CAP as usize - 4096] = 9; // last MiB differs
    a.swap(b, false);
    assert_eq!(c.boundary(), Err(Refusal::FingerprintMismatch));
}

/// THE LIMIT (relay #88): an unreported replacement with the same identity
/// and the same last MiB, whose first MiB differs only in the sectors W2 is
/// about to write, swapped in before W2. W2 overwrites the difference, every
/// boundary passes, and the install completes on the replacement. This KAT
/// asserts that outcome on purpose: a later claim of a stronger bound must
/// fail it.
#[test]
fn masked_unreported_swap_is_not_detected() {
    let a = Device::new(medium("A-0001", 0));
    let reg = one(&a);
    let mut c = claim(a.clone(), &reg.enumerate(0), ClaimKind::Fresh, &reg.ctx(0)).unwrap();
    w1(&mut c);
    c.boundary().unwrap();
    // B: A's identity, A's last MiB after W1, and a first MiB that differs
    // only inside sectors 0..34 (what W2 writes).
    let b = medium("A-0001", 0);
    b.bytes.borrow_mut()[(CAP - S) as usize..].copy_from_slice(&[0xb1; S as usize]);
    b.bytes.borrow_mut()[17 * S as usize] = 0x77;
    let b_interior = 4 * WINDOW_BYTES as usize;
    b.bytes.borrow_mut()[b_interior] = 0x42; // B's own data, outside the windows
    a.swap(b.clone(), false);
    w2(&mut c);
    assert_eq!(c.boundary(), Ok(()), "the limit: not detected");
    // The install goes on, onto B's interior.
    c.write(b_interior as u64, &[0x33; 512]).unwrap();
    c.boundary().unwrap();
    assert_eq!(b.bytes.borrow()[b_interior], 0x33, "B was written");
}

/// The boot disk and the protected-disk table are refused, with no override.
#[test]
fn boot_disk_and_protected_disks_are_refused() {
    let a = Device::new(medium("WIN-NVME-0001", 0));
    let reg = one(&a);
    let mut seen = reg.enumerate(0);
    let protected = ["WIN-NVME-0001"];
    let ctx = ClaimContext {
        protected: &protected,
        ..reg.ctx(0)
    };
    for kind in [ClaimKind::Fresh, ClaimKind::Destructive] {
        assert_eq!(
            claim(a.clone(), &seen, kind, &ctx).err(),
            Some(Refusal::Protected)
        );
    }
    seen.disk_guid = Some([0x4c; 16]);
    let ctx = ClaimContext {
        boot_disk_guid: Some([0x4c; 16]),
        ..reg.ctx(0)
    };
    assert_eq!(
        claim(a.clone(), &seen, ClaimKind::Fresh, &ctx).err(),
        Some(Refusal::BootDisk)
    );
    assert_eq!(a.writes.get(), 0);
}

/// One claim per device at a time.
#[test]
fn a_second_claim_is_refused() {
    let a = Device::new(medium("A-0001", 0));
    let reg = one(&a);
    let seen = reg.enumerate(0);
    let ctx = ClaimContext {
        already_claimed: true,
        ..reg.ctx(0)
    };
    assert_eq!(
        claim(a, &seen, ClaimKind::Fresh, &ctx).err(),
        Some(Refusal::AlreadyClaimed)
    );
}

/// A write past the device is refused without touching it, and does not stop
/// the claim; a dead device stops it.
#[test]
fn out_of_range_and_dead_device() {
    let a = Device::new(medium("A-0001", 0));
    let reg = one(&a);
    let mut c = claim(a.clone(), &reg.enumerate(0), ClaimKind::Fresh, &reg.ctx(0)).unwrap();
    assert_eq!(c.write(CAP - 256, &[0; 512]), Err(Refusal::OutOfRange));
    assert_eq!(c.write(u64::MAX, &[0; 1]), Err(Refusal::OutOfRange));
    assert_eq!(c.stopped(), None);
    a.alive.set(false);
    assert_eq!(c.write(0, &[0; 512]), Err(Refusal::DeviceDead));
    assert_eq!(c.write(0, &[0; 512]), Err(Refusal::Stopped));
    assert_eq!(a.writes.get(), 0);
}

/// A flush is held to the same checks as a write, and reads never write.
#[test]
fn flush_and_read_are_checked() {
    let a = Device::new(medium("A-0001", 0));
    let reg = one(&a);
    let mut c = claim(a.clone(), &reg.enumerate(0), ClaimKind::Fresh, &reg.ctx(0)).unwrap();
    w1(&mut c);
    c.flush().unwrap();
    let mut back = [0u8; 512];
    c.read(CAP - S, &mut back).unwrap();
    assert_eq!(back, [0xb1; 512]);
    assert_eq!(c.read(CAP, &mut back), Err(Refusal::OutOfRange));
    a.swap(medium("A-0001", 0), true);
    assert_eq!(c.flush(), Err(Refusal::MediaChangedBefore));
    assert_eq!(c.read(0, &mut back), Err(Refusal::Stopped));
    assert_eq!(a.writes.get(), 1);
}

/// The expected windows overlay exactly the bytes written into them, and a
/// device smaller than two windows overlaps them correctly.
#[test]
fn windows_overlay() {
    let mut w = Windows {
        capacity: 3 * WINDOW_BYTES,
        first: vec![0; WINDOW_BYTES as usize],
        last: vec![0; WINDOW_BYTES as usize],
    };
    w.overlay(WINDOW_BYTES - 2, &[1, 2, 3, 4]); // straddles the first window's end
    assert_eq!(&w.first[WINDOW_BYTES as usize - 2..], &[1, 2]);
    assert!(w.last.iter().all(|&b| b == 0));
    w.overlay(3 * WINDOW_BYTES - 1, &[9]);
    assert_eq!(w.last[WINDOW_BYTES as usize - 1], 9);
    // A 1.5 MiB device: the windows overlap in the middle half MiB.
    let cap = WINDOW_BYTES * 3 / 2;
    let mut w = Windows {
        capacity: cap,
        first: vec![0; WINDOW_BYTES as usize],
        last: vec![0; WINDOW_BYTES as usize],
    };
    w.overlay(WINDOW_BYTES - 1, &[7]);
    assert_eq!(w.first[WINDOW_BYTES as usize - 1], 7);
    assert_eq!(
        w.last[(WINDOW_BYTES - 1 - (cap - WINDOW_BYTES)) as usize],
        7
    );
}

/// Enumeration's digests, taken a chunk at a time, equal the digests of the
/// whole windows; the chunked compare at the boundary sees a one-byte change
/// in the last chunk of either window.
#[test]
fn fingerprint_and_chunked_compare_agree_with_the_windows() {
    let m = medium("SER-A", 0x5a);
    m.bytes.borrow_mut()[7] = 1;
    m.bytes.borrow_mut()[CAP as usize - 3] = 2;
    let d = Device::new(m.clone());
    let device: &dyn ClaimTarget = &d;
    let w = Windows::read(device).unwrap();
    assert_eq!(Fingerprint::read(device).unwrap(), w.fingerprint());
    assert_eq!(w.matches(device), Some(true));
    for at in [WINDOW_BYTES as usize - 1, CAP as usize - 1] {
        m.bytes.borrow_mut()[at] ^= 0xff;
        assert_ne!(Fingerprint::read(device).unwrap(), w.fingerprint());
        assert_eq!(w.matches(device), Some(false));
        m.bytes.borrow_mut()[at] ^= 0xff;
    }
    assert_eq!(w.matches(device), Some(true));
}

/// Muse's R4.1 review, item 1: a claim records whether any write was
/// submitted (the kernel then moves the slot to a new generation when the
/// writer drops, so the enumeration expires with a used claim). A write
/// refused before submission doesn't count.
#[test]
fn a_claim_records_whether_it_wrote() {
    let a = Device::new(medium("A-0001", 0));
    let reg = one(&a);
    let mut c = claim(a.clone(), &reg.enumerate(0), ClaimKind::Fresh, &reg.ctx(0)).unwrap();
    assert!(!c.wrote());
    a.media_generation.set(a.media_generation.get() + 1);
    assert_eq!(
        c.write(0, &[1; S as usize]),
        Err(Refusal::MediaChangedBefore)
    );
    assert!(!c.wrote(), "refused before submission");
    let b = Device::new(medium("B-0001", 0));
    let reg = one(&b);
    let mut c = claim(b.clone(), &reg.enumerate(0), ClaimKind::Fresh, &reg.ctx(0)).unwrap();
    c.write(0, &[1; S as usize]).unwrap();
    assert!(c.wrote());
}
