// SPDX-License-Identifier: MPL-2.0
//! The cut KATs. Each script is run once without a fault to count its disk
//! events; then it is re-run with power cut just before every event, once for
//! every subset of the unflushed writes landing (all subsets up to
//! `EXHAUSTIVE_PENDING` unflushed writes, a fixed pseudo-random sample
//! beyond), and with every flush failing after landing every subset. After
//! each, the media must mount, at a generation the volume published, with
//! exactly that generation's files.

use std::collections::BTreeMap;

use crate::commit_logic::{DeferredFrees, DrainStep, Role, SuperblockDurability, next_drain_step};
use crate::model::{Content, Disk, DiskError, Fault, Order, Volume, mount};

/// Enumerate every landing subset when at most this many writes are unflushed.
const EXHAUSTIVE_PENDING: usize = 12;
/// Subsets sampled per event beyond that.
const SAMPLES: u64 = 4096;

type Script = fn(&mut Volume, &mut Disk) -> Result<(), DiskError>;

struct Outcome {
    cases: usize,
    failures: Vec<String>,
}

/// `frees`: commits free what they supersede (the write-leak fix, as the
/// kernel does now); off is `main` before it.
fn run(order: Order, defer: bool, frees: bool, script: Script, fault: Fault) -> (Volume, Disk) {
    let (mut volume, media) = Volume::format(order, defer, frees);
    let mut disk = Disk::new(media, fault);
    let _ = script(&mut volume, &mut disk);
    disk.power_off();
    (volume, disk)
}

fn check(volume: &Volume, media: &BTreeMap<u64, Content>) -> Result<(), String> {
    let Some((generation, files)) = mount(media) else {
        return Err("no superblock copy mounts".into());
    };
    match volume.history.get(&generation) {
        Some(expected) if *expected == files => Ok(()),
        Some(_) => Err(format!("generation {generation} mounts with other files")),
        None => Err(format!("generation {generation} was never published")),
    }
}

/// xorshift64*, for the sampled subsets.
fn next(state: &mut u64) -> u64 {
    *state ^= *state >> 12;
    *state ^= *state << 25;
    *state ^= *state >> 27;
    state.wrapping_mul(0x2545_f491_4f6c_dd1d)
}

fn masks(pending: usize, seed: u64) -> Vec<u64> {
    if pending <= EXHAUSTIVE_PENDING {
        return (0..1u64 << pending).collect();
    }
    let all = if pending >= 64 {
        u64::MAX
    } else {
        (1 << pending) - 1
    };
    let mut state = seed | 1;
    let mut out = vec![0, all];
    out.extend((0..SAMPLES).map(|_| next(&mut state) & all));
    out
}

fn every_cut(order: Order, defer: bool, frees: bool, script: Script) -> Outcome {
    let (_, dry) = run(order, defer, frees, script, Fault::None);
    let mut outcome = Outcome {
        cases: 0,
        failures: Vec::new(),
    };
    for at in 0..dry.events {
        for land in masks(dry.pending_at_event[at], at as u64) {
            let (volume, disk) = run(order, defer, frees, script, Fault::Cut { at, land });
            outcome.cases += 1;
            if let Err(e) = check(&volume, &disk.media) {
                outcome
                    .failures
                    .push(format!("cut before event {at}, land {land:#x}: {e}"));
            }
        }
    }
    for &at in &dry.flush_events {
        for land in masks(dry.pending_at_event[at], 0x5eed ^ at as u64) {
            let (volume, disk) = run(order, defer, frees, script, Fault::FlushFails { at, land });
            outcome.cases += 1;
            if let Err(e) = check(&volume, &disk.media) {
                outcome
                    .failures
                    .push(format!("flush {at} fails, land {land:#x}: {e}"));
            }
        }
    }
    outcome
}

// ---- The scripts: a commit's create, write (overwrite) and truncate. ------

fn create_then_overwrite(v: &mut Volume, d: &mut Disk) -> Result<(), DiskError> {
    v.write("f", b"version-a");
    v.drain(d)?;
    v.write("f", b"version-b!");
    v.drain(d)
}

/// Several commits in the cache at once, drained together (the write-back
/// worker paused, then one fsync).
fn batched_commits(v: &mut Volume, d: &mut Disk) -> Result<(), DiskError> {
    v.write("f", b"version-a");
    v.drain(d)?;
    v.write("f", b"version-b!");
    v.write("g", b"second");
    v.drain(d)
}

/// Truncate frees the old tree in a second commit; a new file is written in
/// the same batch, so without deferred reuse it takes the freed blocks.
fn truncate_then_create(v: &mut Volume, d: &mut Disk) -> Result<(), DiskError> {
    v.write("f", b"abcdefgh");
    v.drain(d)?;
    v.truncate("f", 2);
    v.write("g", b"ZZZZZZZZ");
    v.drain(d)
}

/// The same, drained after the truncate: once the freeing commit is durable
/// the blocks may be, and are, reused.
fn truncate_drain_then_create(v: &mut Volume, d: &mut Disk) -> Result<(), DiskError> {
    v.write("f", b"abcdefgh");
    v.drain(d)?;
    v.truncate("f", 2);
    v.drain(d)?;
    v.write("g", b"ZZZZZZZZ");
    v.drain(d)
}

/// The write-leak fix's own script: overwrites drained one at a time and
/// batched, so every superseded version (file, directory, root, free map) is
/// freed, deferred, and its blocks reused once durable, with a second file
/// written alongside.
fn write_then_write(v: &mut Volume, d: &mut Disk) -> Result<(), DiskError> {
    v.write("f", b"version-1");
    v.drain(d)?;
    v.write("f", b"version-2!");
    v.write("f", b"v3");
    v.drain(d)?;
    v.write("g", b"other");
    v.write("f", b"version-four");
    v.drain(d)
}

const SCRIPTS: [(&str, Script); 5] = [
    ("create_then_overwrite", create_then_overwrite),
    ("batched_commits", batched_commits),
    ("truncate_then_create", truncate_then_create),
    ("truncate_drain_then_create", truncate_drain_then_create),
    ("write_then_write", write_then_write),
];

// ---- The KATs. --------------------------------------------------------------

/// The fix: with the barrier and deferred reuse, every cut and every failed
/// flush of every script leaves a generation that mounts with its own files,
/// with commits freeing what they supersede (the kernel now) and without
/// (`main` before the write-leak fix).
#[test]
fn barrier_and_deferred_reuse_survive_every_cut() {
    for frees in [true, false] {
        for (name, script) in SCRIPTS {
            let out = every_cut(Order::Barrier, true, frees, script);
            assert!(out.cases > 100, "{name}: only {} cases", out.cases);
            assert!(
                out.failures.is_empty(),
                "{name} (frees {frees}): {} of {} cases fail, e.g. {:?}",
                out.failures.len(),
                out.cases,
                &out.failures[..out.failures.len().min(3)]
            );
            println!("{name} (frees {frees}): {} cases, all mount", out.cases);
        }
    }
}

/// The write-leak fix needs deferred reuse too: without it, a superseded
/// version's blocks are written by the next commit before the freeing
/// commit is durable, and a cut that brings the older generation back finds
/// them overwritten.
#[test]
fn freeing_superseded_versions_without_deferred_reuse_loses_the_volume() {
    let out = every_cut(Order::Barrier, false, true, write_then_write);
    println!(
        "frees without deferred reuse: {} of {} cases fail",
        out.failures.len(),
        out.cases
    );
    assert!(!out.failures.is_empty());
}

/// N overwrites of one file keep the used-block count constant once each is
/// durable (its deferred frees reusable); `main` before the fix grows by
/// every superseded block, every write.
#[test]
fn overwrites_keep_the_used_count_constant() {
    for frees in [true, false] {
        let (mut v, media) = Volume::format(Order::Barrier, true, frees);
        let mut disk = Disk::new(media, Fault::None);
        v.write("f", b"first version");
        v.drain(&mut disk).unwrap();
        let mut used = Vec::new();
        for i in 0..50u8 {
            v.write("f", &[b'a' + i % 26; 13]);
            v.drain(&mut disk).unwrap();
            used.push(v.used());
        }
        if frees {
            assert!(used.iter().all(|&u| u == used[0]), "used: {used:?}");
            assert!(v.high_water() < 64, "high water {}", v.high_water());
        } else {
            // Data (7 blocks), inode, directory, root inode and free map.
            assert!(used.windows(2).all(|w| w[1] == w[0] + 11), "used: {used:?}");
        }
        let (_, files) = mount(&disk.media).expect("mounts");
        assert_eq!(files["f"], [b'a' + 49 % 26; 13]);
    }
}

/// `main`'s order: every role in order, one flush at the end. A cut that
/// lands the new superblocks but not the root they name leaves no copy that
/// mounts (Janus #93 item 4).
#[test]
fn the_single_flush_order_loses_the_volume() {
    let out = every_cut(Order::SingleFlush, true, true, create_then_overwrite);
    println!(
        "single flush: {} of {} cases fail",
        out.failures.len(),
        out.cases
    );
    assert!(
        out.failures
            .iter()
            .any(|f| f.ends_with("no superblock copy mounts")),
        "{:?}",
        &out.failures[..out.failures.len().min(3)]
    );
}

/// Without deferred reuse, even with the barrier: the truncate's freed
/// blocks are written with the new file before any newer superblock is
/// durable, so a cut brings back the old generation with its file's blocks
/// overwritten, and mount (which checks file contents) rejects it too.
#[test]
fn reuse_before_the_freeing_commit_is_durable_loses_the_volume() {
    let out = every_cut(Order::Barrier, false, true, truncate_then_create);
    println!(
        "no deferred reuse: {} of {} cases fail",
        out.failures.len(),
        out.cases
    );
    assert!(
        out.failures
            .iter()
            .any(|f| f.ends_with("no superblock copy mounts")),
        "{:?}",
        &out.failures[..out.failures.len().min(3)]
    );
}

/// The deferred blocks are reused once their generation is durable: the
/// script that drains after the truncate allocates them again.
#[test]
fn deferred_blocks_are_reused_once_durable() {
    let (drained, _) = run(
        Order::Barrier,
        true,
        true,
        truncate_drain_then_create,
        Fault::None,
    );
    let (batched, _) = run(
        Order::Barrier,
        true,
        true,
        truncate_then_create,
        Fault::None,
    );
    assert!(
        drained.high_water() < batched.high_water(),
        "drained: {}, batched: {}",
        drained.high_water(),
        batched.high_water()
    );
}

/// With no fault, both orders leave the last generation mountable.
#[test]
fn a_clean_run_mounts_the_last_generation() {
    for order in [Order::SingleFlush, Order::Barrier] {
        let (v, disk) = run(order, true, true, batched_commits, Fault::None);
        let (generation, files) = mount(&disk.media).expect("mounts");
        assert_eq!(generation, *v.history.keys().last().unwrap());
        assert_eq!(files["f"], b"version-b!");
        assert_eq!(files["g"], b"second");
    }
}

// ---- commit_logic on its own. -----------------------------------------------

#[test]
fn the_drain_flushes_before_a_superblock_while_data_is_volatile() {
    let dirty = [(0, Role::Superblock), (1, Role::Superblock)];
    assert_eq!(next_drain_step(dirty, true, true), DrainStep::Flush);
    assert_eq!(next_drain_step(dirty, false, true), DrainStep::Write(0));
    let mixed = [(0, Role::Superblock), (1, Role::Data)];
    assert_eq!(next_drain_step(mixed, true, true), DrainStep::Write(1));
    assert_eq!(next_drain_step([], false, true), DrainStep::Flush);
    assert_eq!(next_drain_step([], true, false), DrainStep::Done);
    assert_eq!(next_drain_step([], false, false), DrainStep::Done);
}

#[test]
fn durability_is_the_oldest_copy_proven_by_a_flush() {
    let mut d = SuperblockDurability::new([true, true, false], 5);
    assert_eq!(d.durable_generation(), 5);
    d.written(0, 6);
    d.written(1, 6);
    assert_eq!(d.durable_generation(), 5, "written, not flushed");
    d.flushed();
    assert_eq!(d.durable_generation(), 6, "copy 2 is absent");
    d.written(0, 7);
    d.flushed();
    assert_eq!(d.durable_generation(), 6, "copy 1 is still at 6");
}

#[test]
fn deferred_frees_release_at_their_generation() {
    let mut f = DeferredFrees::new();
    f.defer(10, 7);
    f.defer(11, 8);
    f.release(6);
    assert!(f.is_deferred(10) && f.is_deferred(11));
    f.release(7);
    assert!(!f.is_deferred(10) && f.is_deferred(11));
    f.release(8);
    assert!(f.is_empty());
}

/// A commit whose publish failed takes its frees back (`unfree`).
#[test]
fn a_cancelled_free_is_no_longer_deferred() {
    let mut f = DeferredFrees::new();
    f.defer(10, 7);
    f.defer(11, 7);
    f.cancel(10);
    assert!(!f.is_deferred(10) && f.is_deferred(11));
    f.cancel(99);
    assert_eq!(f.len(), 1);
}
