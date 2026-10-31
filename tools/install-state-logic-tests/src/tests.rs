// SPDX-License-Identifier: MPL-2.0
//! The §4.7 KATs: every cut (each submission and each flush), every failed
//! flush, small cases exhaustively by write call and the rest by seeded
//! sector subsets, and the three named traces.
//!
//! The invariant after every outcome: the disk classifies as blank,
//! LogansOS incomplete or LogansOS installed (never foreign: never stranded),
//! "installed" implies every payload byte is on the media, every GPT header
//! was submitted only after its array was durable, and nothing was submitted
//! after a failed flush.

use std::collections::BTreeMap;

use crate::gpt_logic;
use crate::install_logic::*;
use crate::model::{Fault, Land, Model};

const GUIDS: Guids = Guids {
    disk: [0x11; 16],
    esp: [0x22; 16],
    neofs: [0x33; 16],
};

/// A 4 GiB disk at sector size `s` (§4.4's oracle geometry).
fn geometry(s: u64) -> (u64, u64) {
    let capacity = 4u64 << 30;
    (capacity, capacity / s - 1)
}

/// W3–W5's stand-in: a few calls into each partition, each step's bytes
/// distinct, and an extra flush inside W4 and W5 (the NeoFS grow and sync).
struct FakePayload {
    regions: Vec<(Step, u64, Vec<u8>)>,
}

impl FakePayload {
    fn new(tables: &Tables) -> Self {
        let s = tables.complete.sector_size;
        let esp = tables.complete.esp.0;
        let neofs = tables.complete.neofs.0;
        let fill = |step: u8, n: u64| vec![0x40 + step; (n * s) as usize];
        Self {
            regions: vec![
                (Step::W3, esp, fill(3, 2)),
                (Step::W3, esp + 40, fill(3, 1)),
                (Step::W3, esp + 80, fill(3, 3)),
                (Step::W4, neofs, fill(4, 2)),
                (Step::W4, neofs + 16, fill(4, 1)),
                (Step::W5, neofs + 64, fill(5, 2)),
                (Step::W5, neofs + 96, fill(5, 1)),
            ],
        }
    }

    /// Every payload byte is durable on `m`.
    fn on_media(&self, m: &Model) -> bool {
        self.regions.iter().all(|(_, lba, bytes)| {
            bytes
                .chunks(m.sector as usize)
                .enumerate()
                .all(|(i, c)| m.media.get(&(lba + i as u64)).map(|v| v.as_slice()) == Some(c))
        })
    }
}

impl Payload<Model> for FakePayload {
    fn write(&mut self, step: Step, t: &mut Model) -> Result<(), Failure> {
        let mine: Vec<_> = self.regions.iter().filter(|r| r.0 == step).collect();
        for (i, (_, lba, bytes)) in mine.iter().enumerate() {
            if !t.write(*lba, bytes) {
                return Err(Failure::Write { step, lba: *lba });
            }
            if i == 0 && step != Step::W3 && !t.flush() {
                return Err(Failure::Flush { step });
            }
        }
        Ok(())
    }

    fn verify(&mut self, step: Step, t: &mut Model) -> Result<(), Failure> {
        for (_, lba, bytes) in self.regions.iter().filter(|r| r.0 == step) {
            verify(t, step, *lba, bytes)?;
        }
        Ok(())
    }
}

fn tables(s: u64) -> Tables {
    let (_, l) = geometry(s);
    Tables::new(s, l, GUIDS).unwrap()
}

/// A model whose GPT headers are watched for the array-first barrier.
fn model(s: u64, fault: Fault, media: BTreeMap<u64, Vec<u8>>) -> Model {
    let (capacity, l) = geometry(s);
    let t = tables(s);
    let a = gpt_logic::LAYOUT_ARRAY_BYTES as u64 / s;
    let mut m = Model::resume(s, capacity, media, fault);
    m.header_arrays = vec![(l, t.complete.backup_entries_lba, a), (1, 2, a)];
    m
}

fn run(
    s: u64,
    fault: Fault,
    media: BTreeMap<u64, Vec<u8>>,
    start: Start,
    order: Order,
) -> (Model, FakePayload, Result<Outcome, Failure>) {
    let t = tables(s);
    let mut m = model(s, fault, media);
    let mut p = FakePayload::new(&t);
    let r = install_ordered(&mut m, &t, &mut p, start, None, order);
    (m, p, r)
}

/// The invariant every outcome must meet.
fn check(m: &Model, p: &FakePayload, r: &Result<Outcome, Failure>, what: &str) {
    assert!(
        !m.wrote_after_failed_flush,
        "{what}: a write after a failed flush"
    );
    assert!(
        m.header_submissions.iter().all(|(_, durable)| *durable),
        "{what}: a header went down before its array was durable: {:?}",
        m.header_submissions
    );
    let class = classify(&m.image());
    match class {
        Class::Blank | Class::Incomplete(GUIDS) => {}
        Class::Installed(GUIDS) => assert!(p.on_media(m), "{what}: installed without its payload"),
        other => panic!("{what}: stranded as {other:?} after {r:?}"),
    }
    if r == &Ok(Outcome::Installed) {
        assert!(
            matches!(class, Class::Installed(_)),
            "{what}: reported installed, reads {class:?}"
        );
    }
}

/// The landing choices tried at each fault: every subset of the first four
/// pending calls, then sector-level seeds.
fn landings() -> Vec<Land> {
    let mut out: Vec<Land> = (0..16).map(Land::Calls).collect();
    out.push(Land::Calls(u64::MAX));
    out.extend((0..12).map(Land::Seeded));
    out
}

fn events(s: u64, media: &BTreeMap<u64, Vec<u8>>, start: Start) -> usize {
    let (m, p, r) = run(s, Fault::None, media.clone(), start, Order::ArrayFirst);
    assert_eq!(r, Ok(Outcome::Installed));
    check(&m, &p, &r, "no fault");
    m.events
}

#[test]
fn a_clean_install_writes_both_tables_complete_and_the_payload() {
    for s in [512, 4096] {
        let (m, p, r) = run(
            s,
            Fault::None,
            BTreeMap::new(),
            Start::Fresh,
            Order::ArrayFirst,
        );
        assert_eq!(r, Ok(Outcome::Installed));
        assert_eq!(classify(&m.image()), Class::Installed(GUIDS));
        assert!(p.on_media(&m));
        // Both copies validate on their own, complete.
        let t = tables(s);
        let l = t.complete.last_lba;
        let img = m.image();
        let primary = gpt_logic::read_header(&img, 1, s)
            .and_then(|h| gpt_logic::validate(&img, &h, 1, s, m.capacity))
            .unwrap();
        let backup = gpt_logic::read_header(&img, l, s)
            .and_then(|h| gpt_logic::validate(&img, &h, l, s, m.capacity))
            .unwrap();
        for gpt in [primary, backup] {
            assert_eq!(gpt.entries[1].attributes & gpt_logic::ATTR_INCOMPLETE, 0);
        }
        // One boundary check per step.
        assert_eq!(m.boundaries, 6);
        // The bytes are the layout's, byte for byte (the R3 oracle's).
        let sector = |lba: u64| m.media.get(&lba).cloned().unwrap();
        assert_eq!(sector(0), t.complete.mbr);
        assert_eq!(sector(1), t.complete.primary_header);
        assert_eq!(sector(l), t.complete.backup_header);
    }
}

/// Every cut and every failed flush, from a blank disk, at both sector sizes.
#[test]
fn every_cut_and_every_failed_flush_leaves_a_classifiable_disk() {
    for s in [512, 4096] {
        let total = events(s, &BTreeMap::new(), Start::Fresh);
        let mut seen = std::collections::HashSet::new();
        for at in 0..total {
            for land in landings() {
                let (m, p, r) = run(
                    s,
                    Fault::Cut { at, land },
                    BTreeMap::new(),
                    Start::Fresh,
                    Order::ArrayFirst,
                );
                check(&m, &p, &r, &format!("S={s} cut at {at} landing {land:?}"));
                assert!(r.is_err(), "S={s}: a cut at {at} reported {r:?}");
                seen.insert(std::mem::discriminant(&classify(&m.image())));
            }
        }
        // The sweep reached all three outcomes.
        assert_eq!(seen.len(), 3, "S={s}");
        // A failed flush at each flush event.
        for at in flush_events(s, &BTreeMap::new(), Start::Fresh) {
            for land in landings() {
                let (m, p, r) = run(
                    s,
                    Fault::FlushFails { at, land },
                    BTreeMap::new(),
                    Start::Fresh,
                    Order::ArrayFirst,
                );
                check(
                    &m,
                    &p,
                    &r,
                    &format!("S={s} flush {at} fails landing {land:?}"),
                );
                assert!(
                    matches!(r, Err(Failure::Flush { .. })),
                    "S={s} flush {at}: {r:?}"
                );
            }
        }
    }
}

/// The event indices that are flushes, from a fault-free run: a flush is the
/// only event a failed-flush fault may name, so find them by trying.
fn flush_events(s: u64, media: &BTreeMap<u64, Vec<u8>>, start: Start) -> Vec<usize> {
    let total = events(s, media, start);
    (0..total)
        .filter(|&at| {
            std::panic::catch_unwind(|| {
                let t = tables(s);
                let mut m = model(
                    s,
                    Fault::FlushFails {
                        at,
                        land: Land::Calls(0),
                    },
                    media.clone(),
                );
                let mut p = FakePayload::new(&t);
                let _ = install(&mut m, &t, &mut p, start, None);
            })
            .is_ok()
        })
        .collect()
}

/// Relay #88, header only survives: W1's array is volatile and only the
/// backup header lands. With the array-first barrier it can't happen (the
/// header is submitted after the array's flush succeeded, which `check`
/// asserts on every run). Without the barrier the trace bites: the disk is an
/// opaque, invalid GPT, so it would read as foreign and need an erase.
#[test]
fn header_only_trace_bites_without_the_barrier() {
    for s in [512, 4096] {
        // HeaderWithArray, W1: write array (event 0), write header (1),
        // flush (2). Cut at the flush; only call 1, the header, lands.
        let (m, _, r) = run(
            s,
            Fault::Cut {
                at: 2,
                land: Land::Calls(0b10),
            },
            BTreeMap::new(),
            Start::Fresh,
            Order::HeaderWithArray,
        );
        assert!(r.is_err());
        assert!(
            !m.header_submissions[0].1,
            "the header went down with its array volatile"
        );
        assert_eq!(classify(&m.image()), Class::Foreign, "opaque-gpt-invalid");
        // The same cut under the barrier: W1's events are write array (0),
        // flush (1), write header (2). Cutting at the header's submission
        // lands nothing of it; the disk is still blank.
        let (m, p, r) = run(
            s,
            Fault::Cut {
                at: 2,
                land: Land::Calls(u64::MAX),
            },
            BTreeMap::new(),
            Start::Fresh,
            Order::ArrayFirst,
        );
        check(&m, &p, &r, "barrier");
        assert_eq!(classify(&m.image()), Class::Blank);
    }
}

/// The event index of the flush right after W6's backup header write.
fn w6_backup_header_flush(s: u64) -> usize {
    let t = tables(s);
    let l = t.complete.last_lba;
    // Replay with a tap on writes: count events until the second write to
    // the backup header LBA (W1's, then W6's), then the next event is its
    // flush.
    struct Tap {
        inner: Model,
        writes_to_l: usize,
        at: Option<usize>,
        l: u64,
    }
    impl Target for Tap {
        fn sector_size(&self) -> u64 {
            self.inner.sector_size()
        }
        fn write(&mut self, lba: u64, bytes: &[u8]) -> bool {
            if lba == self.l {
                self.writes_to_l += 1;
                if self.writes_to_l == 2 {
                    self.at = Some(self.inner.events + 1);
                }
            }
            self.inner.write(lba, bytes)
        }
        fn flush(&mut self) -> bool {
            self.inner.flush()
        }
        fn read(&mut self, lba: u64, out: &mut [u8]) -> bool {
            self.inner.read(lba, out)
        }
        fn boundary(&mut self) -> bool {
            self.inner.boundary()
        }
    }
    impl Payload<Tap> for FakePayload {
        fn write(&mut self, step: Step, t: &mut Tap) -> Result<(), Failure> {
            Payload::<Model>::write(self, step, &mut t.inner)
        }
        fn verify(&mut self, step: Step, t: &mut Tap) -> Result<(), Failure> {
            Payload::<Model>::verify(self, step, &mut t.inner)
        }
    }
    let mut tap = Tap {
        inner: model(s, Fault::None, BTreeMap::new()),
        writes_to_l: 0,
        at: None,
        l,
    };
    let mut p = FakePayload::new(&t);
    assert_eq!(
        install(&mut tap, &t, &mut p, Start::Fresh, None),
        Ok(Outcome::Installed)
    );
    tap.at.unwrap()
}

/// W6's backup flush fails after its sectors persisted: the installer stops
/// (rule 4) and the disk reads as incomplete via the primary, payload intact.
/// Then a separate trace: the restarted Finish installing, cut at every event
/// including the primary update, reads incomplete or installed, never worse.
#[test]
fn w6_backup_flush_failure_then_a_restarted_finish() {
    for s in [512, 4096] {
        let at = w6_backup_header_flush(s);
        let (m, p, r) = run(
            s,
            Fault::FlushFails {
                at,
                land: Land::Calls(u64::MAX),
            },
            BTreeMap::new(),
            Start::Fresh,
            Order::ArrayFirst,
        );
        assert_eq!(r, Err(Failure::Flush { step: Step::W6 }));
        check(&m, &p, &r, "W6 backup flush fails");
        assert_eq!(
            classify(&m.image()),
            Class::Incomplete(GUIDS),
            "via the primary"
        );
        // The backup copy, which persisted, already says complete.
        let img = m.image();
        let l = tables(s).complete.last_lba;
        let backup = gpt_logic::read_header(&img, l, s)
            .and_then(|h| gpt_logic::validate(&img, &h, l, s, m.capacity))
            .unwrap();
        assert_eq!(backup.entries[1].attributes & gpt_logic::ATTR_INCOMPLETE, 0);
        assert!(p.on_media(&m));

        // A new session: classify, admit, Finish installing from W3.
        let media = m.media.clone();
        let guids = admit(classify(&m.image()), Start::Finish).unwrap();
        assert_eq!(guids, Some(GUIDS));
        let total = events(s, &media, Start::Finish);
        for at in 0..total {
            for land in landings() {
                let (m, p, r) = run(
                    s,
                    Fault::Cut { at, land },
                    media.clone(),
                    Start::Finish,
                    Order::ArrayFirst,
                );
                check(&m, &p, &r, &format!("finish S={s} cut at {at}"));
                assert!(
                    matches!(
                        classify(&m.image()),
                        Class::Incomplete(_) | Class::Installed(_)
                    ),
                    "a finish never goes back to blank"
                );
            }
        }
        let (m, _, r) = run(s, Fault::None, media, Start::Finish, Order::ArrayFirst);
        assert_eq!(r, Ok(Outcome::Installed));
        assert_eq!(classify(&m.image()), Class::Installed(GUIDS));
    }
}

/// `install-stop-after=Wn`: stopping after each step leaves the disk as the
/// §4.7 table says, and Finish installing then completes it.
#[test]
fn stop_after_each_step_then_finish() {
    for s in [512, 4096] {
        for step in Step::ALL {
            let t = tables(s);
            let mut m = model(s, Fault::None, BTreeMap::new());
            let mut p = FakePayload::new(&t);
            let r = install(&mut m, &t, &mut p, Start::Fresh, Some(step));
            let class = classify(&m.image());
            if step == Step::W6 {
                assert_eq!(r, Ok(Outcome::Installed));
                assert_eq!(class, Class::Installed(GUIDS));
                continue;
            }
            assert_eq!(r, Ok(Outcome::StoppedAfter(step)));
            // After W1 the backup copy alone makes it incomplete.
            assert_eq!(class, Class::Incomplete(GUIDS), "after {step:?}");
            let guids = admit(class, Start::Finish).unwrap().unwrap();
            let t2 = Tables::new(s, t.complete.last_lba, guids).unwrap();
            let mut m2 = model(s, Fault::None, m.media.clone());
            let mut p2 = FakePayload::new(&t2);
            assert_eq!(
                install(&mut m2, &t2, &mut p2, Start::Finish, None),
                Ok(Outcome::Installed)
            );
            assert_eq!(classify(&m2.image()), Class::Installed(GUIDS));
            assert!(p2.on_media(&m2));
        }
    }
}

/// §4.6 and §4.7 rule 1: refused before any write.
#[test]
fn small_disks_odd_sectors_and_no_flush_are_refused() {
    assert_eq!(check_target(MIN_TARGET_BYTES, 512, true), Ok(()));
    assert_eq!(check_target(MIN_TARGET_BYTES, 4096, true), Ok(()));
    assert_eq!(check_target(1 << 30, 512, true), Err(Refusal::TooSmall));
    // One sector short, at both sizes.
    assert_eq!(
        check_target(MIN_TARGET_BYTES - 512, 512, true),
        Err(Refusal::TooSmall)
    );
    assert_eq!(
        check_target(MIN_TARGET_BYTES - 4096, 4096, true),
        Err(Refusal::TooSmall)
    );
    assert_eq!(
        check_target(MIN_TARGET_BYTES, 1024, true),
        Err(Refusal::SectorSize)
    );
    assert_eq!(
        check_target(MIN_TARGET_BYTES, 520, true),
        Err(Refusal::SectorSize)
    );
    assert_eq!(
        check_target(MIN_TARGET_BYTES, 512, false),
        Err(Refusal::NoFlush)
    );
    // The minimum takes a layout at both sizes.
    for s in [512, 4096] {
        assert!(Tables::new(s, MIN_TARGET_BYTES / s - 1, GUIDS).is_some());
    }
}

/// What may be installed to: blank for a fresh install, our incomplete table
/// for Finish installing. Everything else is refused (installed and foreign
/// disks are R4b's).
#[test]
fn only_blank_and_incomplete_disks_are_admitted() {
    let s = 512;
    let (capacity, _) = geometry(s);
    let blank = Model::new(s, capacity, Fault::None);
    assert_eq!(classify(&blank.image()), Class::Blank);
    assert_eq!(admit(Class::Blank, Start::Fresh), Ok(None));
    assert!(admit(Class::Blank, Start::Finish).is_err());
    assert!(admit(Class::Incomplete(GUIDS), Start::Fresh).is_err());
    assert!(admit(Class::Installed(GUIDS), Start::Fresh).is_err());
    assert!(admit(Class::Installed(GUIDS), Start::Finish).is_err());
    assert!(admit(Class::Foreign, Start::Fresh).is_err());
    // A boot sector or a filesystem in the first MiB: foreign.
    let mut ntfs = Model::new(s, capacity, Fault::None);
    let mut sector = vec![0u8; 512];
    sector[3..11].copy_from_slice(b"NTFS    ");
    ntfs.media.insert(0, sector);
    assert_eq!(classify(&ntfs.image()), Class::Foreign);
    // Data deep in the first MiB counts too.
    let mut data = Model::new(s, capacity, Fault::None);
    data.media.insert(2047, vec![1; 512]);
    assert_eq!(classify(&data.image()), Class::Foreign);
    // Past the first MiB it doesn't (a blank disk may hold old data there).
    let mut old = Model::new(s, capacity, Fault::None);
    old.media.insert(2048, vec![1; 512]);
    assert_eq!(classify(&old.image()), Class::Blank);
    // A GPT signature that doesn't validate: opaque, foreign.
    let mut opaque = Model::new(s, capacity, Fault::None);
    let mut hdr = vec![0u8; 512];
    hdr[..8].copy_from_slice(gpt_logic::SIGNATURE);
    opaque.media.insert(1, hdr);
    assert_eq!(classify(&opaque.image()), Class::Foreign);
    // A valid GPT that isn't ours: our layout for a smaller disk, written
    // onto this one, places the backup elsewhere; then a valid table with a
    // third entry.
    let other = Tables::new(s, (3u64 << 30) / s - 1, GUIDS)
        .unwrap()
        .complete;
    let mut moved = Model::new(s, 3u64 << 30, Fault::None);
    moved.media.insert(0, other.mbr.clone());
    moved.media.insert(1, other.primary_header.clone());
    for (i, c) in other.entries.chunks(512).enumerate() {
        moved.media.insert(2 + i as u64, c.to_vec());
    }
    assert_eq!(
        classify(&moved.image()),
        Class::Installed(GUIDS),
        "same layout, its own disk"
    );
    let mut bigger = Model::new(s, capacity, Fault::None);
    bigger.media = moved.media.clone();
    // On a 4 GiB disk, the 3 GiB disk's primary still validates (its own LBAs
    // are inside), but its partition 2 isn't where this disk's layout puts it.
    assert_eq!(classify(&bigger.image()), Class::Foreign);
}

/// The sweep bites: under the unbarriered order (each header in its array's
/// flush window) the same cuts strand disks as foreign, or show an install
/// whose table says complete before its array does. So a clean sweep above is
/// evidence, not a sweep that can't fail.
#[test]
fn the_sweep_finds_the_unbarriered_order_unsafe() {
    for s in [512, 4096] {
        let (m, _, r) = run(
            s,
            Fault::None,
            BTreeMap::new(),
            Start::Fresh,
            Order::HeaderWithArray,
        );
        assert_eq!(r, Ok(Outcome::Installed));
        let total = m.events;
        let mut stranded = 0;
        for at in 0..total {
            for land in landings() {
                let (m, _, _) = run(
                    s,
                    Fault::Cut { at, land },
                    BTreeMap::new(),
                    Start::Fresh,
                    Order::HeaderWithArray,
                );
                if classify(&m.image()) == Class::Foreign {
                    stranded += 1;
                }
            }
        }
        assert!(
            stranded > 0,
            "S={s}: the counter-model never stranded a disk"
        );
    }
}
