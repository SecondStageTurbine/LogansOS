// SPDX-License-Identifier: MPL-2.0
//! The cut KATs. Each script builds a small tree, drains it, then runs one
//! directory operation and drains again. It is run once without a fault to
//! count its disk events, then again with power cut just before every event,
//! once for every subset of the unflushed writes landing (every subset up to
//! `EXHAUSTIVE_PENDING` unflushed writes, a fixed sample beyond). After each
//! cut the media must mount at a generation the volume published, with
//! exactly that generation's tree; and a cut during the operation must mount
//! the tree from just before it or just after it, never anything between.

use std::collections::BTreeMap;

use crate::model::{Content, Disk, DiskError, Fault, Order, Refused, Tree, Volume, mount};

const EXHAUSTIVE_PENDING: usize = 12;
const SAMPLES: u64 = 2048;

/// The setup (drained), then the operation under test (drained).
struct Script {
    name: &'static str,
    setup: fn(&mut Volume) -> Result<(), Refused>,
    op: fn(&mut Volume) -> Result<(), Refused>,
}

fn run(order: Order, script: &Script, fault: Fault) -> (Volume, Disk, Option<u64>) {
    let (mut volume, media) = Volume::format(order);
    let mut disk = Disk::new(media, fault);
    let mut before_op = None;
    let result = (|| -> Result<(), DiskError> {
        (script.setup)(&mut volume).expect("setup is valid");
        volume.drain(&mut disk)?;
        before_op = volume.history.keys().next_back().copied();
        (script.op)(&mut volume).expect("the operation is valid");
        volume.drain(&mut disk)
    })();
    let _ = result;
    disk.power_off();
    (volume, disk, before_op)
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

fn check(
    volume: &Volume,
    media: &BTreeMap<u64, Content>,
    before_op: Option<u64>,
) -> Result<(), String> {
    let Some((generation, tree)) = mount(media) else {
        return Err("no superblock copy mounts".into());
    };
    let Some(expected) = volume.history.get(&generation) else {
        return Err(format!("generation {generation} was never published"));
    };
    if *expected != tree {
        return Err(format!("generation {generation} mounts with another tree"));
    }
    // Once the operation began, only the tree before it or after it.
    if let Some(before) = before_op
        && generation != before
        && generation != before + 1
    {
        return Err(format!(
            "generation {generation} is neither {before} (before) nor {} (after)",
            before + 1
        ));
    }
    Ok(())
}

struct Outcome {
    cases: usize,
    failures: Vec<String>,
}

fn every_cut(order: Order, script: &Script) -> Outcome {
    let (_, dry, _) = run(order, script, Fault::None);
    let mut outcome = Outcome {
        cases: 0,
        failures: Vec::new(),
    };
    for at in 0..dry.events {
        for land in masks(dry.pending_at_event[at], at as u64) {
            let (volume, disk, before_op) = run(order, script, Fault::Cut { at, land });
            outcome.cases += 1;
            if let Err(e) = check(&volume, &disk.media, before_op) {
                outcome.failures.push(format!(
                    "{}: cut before event {at}, land {land:#x}: {e}",
                    script.name
                ));
            }
        }
    }
    outcome
}

fn tree_of(entries: &[(&str, Option<&[u8]>)]) -> Tree {
    entries
        .iter()
        .map(|(path, bytes)| (path.to_string(), bytes.map(<[u8]>::to_vec)))
        .collect()
}

fn base(v: &mut Volume) -> Result<(), Refused> {
    v.mkdir("", "a")?;
    v.mkdir("a", "b")?;
    v.mkdir("", "c")?;
    v.create("a/b", "note", b"hello")?;
    Ok(())
}

const SCRIPTS: &[Script] = &[
    Script {
        name: "mkdir_nested",
        setup: base,
        op: |v| v.mkdir("a/b", "new"),
    },
    Script {
        name: "unlink_file",
        setup: base,
        op: |v| v.unlink("a/b", "note"),
    },
    Script {
        name: "unlink_empty_dir",
        setup: base,
        op: |v| v.unlink("", "c"),
    },
    Script {
        name: "rename_in_place",
        setup: base,
        op: |v| v.rename("a", "b", "a", "renamed"),
    },
    Script {
        // Both sides have a tail below the shared root.
        name: "rename_across",
        setup: base,
        op: |v| v.rename("a/b", "note", "c", "moved"),
    },
    Script {
        // A folder with a file, up a level: the source has the tail.
        name: "rename_dir_up",
        setup: base,
        op: |v| v.rename("a", "b", "", "b"),
    },
    Script {
        // Down a level: the destination has the tail.
        name: "rename_dir_down",
        setup: base,
        op: |v| v.rename("", "c", "a/b", "c"),
    },
];

#[test]
fn every_operation_ends_before_or_after_under_every_cut() {
    let mut cases = 0;
    let mut failures = Vec::new();
    for script in SCRIPTS {
        let outcome = every_cut(Order::Barrier, script);
        assert!(
            outcome.cases > 100,
            "{}: only {} cases",
            script.name,
            outcome.cases
        );
        cases += outcome.cases;
        failures.extend(outcome.failures);
    }
    assert!(
        failures.is_empty(),
        "{} of {cases} cuts failed; first: {:?}",
        failures.len(),
        &failures[..failures.len().min(5)]
    );
}

/// The falsifier: without the barrier, a cut can land a superblock before
/// the directories it names, and some operation mounts nothing (or a tree
/// never published). The check has teeth.
#[test]
fn without_the_barrier_a_cut_breaks_some_operation() {
    let failed = SCRIPTS
        .iter()
        .any(|script| !every_cut(Order::SingleFlush, script).failures.is_empty());
    assert!(
        failed,
        "no cut failed without the barrier: the check is vacuous"
    );
}

#[test]
fn the_operations_make_the_trees_they_should() {
    let note: &[u8] = b"hello";
    let base_tree = tree_of(&[
        ("a", None),
        ("a/b", None),
        ("a/b/note", Some(note)),
        ("c", None),
    ]);
    let expect = [
        (
            "mkdir_nested",
            tree_of(&[
                ("a", None),
                ("a/b", None),
                ("a/b/note", Some(note)),
                ("a/b/new", None),
                ("c", None),
            ]),
        ),
        (
            "unlink_file",
            tree_of(&[("a", None), ("a/b", None), ("c", None)]),
        ),
        (
            "unlink_empty_dir",
            tree_of(&[("a", None), ("a/b", None), ("a/b/note", Some(note))]),
        ),
        (
            "rename_in_place",
            tree_of(&[
                ("a", None),
                ("a/renamed", None),
                ("a/renamed/note", Some(note)),
                ("c", None),
            ]),
        ),
        (
            "rename_across",
            tree_of(&[
                ("a", None),
                ("a/b", None),
                ("c", None),
                ("c/moved", Some(note)),
            ]),
        ),
        (
            "rename_dir_up",
            tree_of(&[
                ("a", None),
                ("b", None),
                ("b/note", Some(note)),
                ("c", None),
            ]),
        ),
        (
            "rename_dir_down",
            tree_of(&[
                ("a", None),
                ("a/b", None),
                ("a/b/note", Some(note)),
                ("a/b/c", None),
            ]),
        ),
    ];
    for (script, (name, want)) in SCRIPTS.iter().zip(expect.iter()) {
        assert_eq!(script.name, *name);
        let (volume, disk, before) = run(Order::Barrier, script, Fault::None);
        let before = before.expect("the setup published");
        assert_eq!(
            volume.history[&before], base_tree,
            "{name}: the setup's tree"
        );
        let (generation, tree) = mount(&disk.media).expect("mounts");
        assert_eq!(generation, before + 1, "{name}: one commit");
        assert_eq!(&tree, want, "{name}");
    }
}

/// Every block an operation supersedes is freed in its publish: after a
/// mkdir and its unlink, and a create and its unlink, the volume uses what
/// it used before (the kernel scenario's `used_back`).
#[test]
fn an_unlink_gives_back_what_its_create_took() {
    let (mut v, _media) = Volume::format(Order::Barrier);
    base(&mut v).unwrap();
    let used = v.used();
    v.mkdir("a", "tmp").unwrap();
    v.unlink("a", "tmp").unwrap();
    v.create("c", "x", b"bytes").unwrap();
    v.unlink("c", "x").unwrap();
    assert_eq!(v.used(), used);
    // A rename moves an entry and frees only the old directory versions.
    v.rename("a/b", "note", "c", "note").unwrap();
    assert_eq!(v.used(), used);
}

#[test]
fn the_model_refuses_what_the_kernel_refuses() {
    let (mut v, _media) = Volume::format(Order::Barrier);
    base(&mut v).unwrap();
    assert_eq!(v.unlink("a", "b"), Err(Refused::NotEmpty));
    assert_eq!(v.mkdir("a", "b"), Err(Refused::Exists));
    assert_eq!(v.rename("", "a", "c", "a"), Ok(()));
    assert_eq!(v.rename("c", "a", "c/a/b", "x"), Err(Refused::IntoItself));
    assert_eq!(v.rename("c", "a", "c", "a"), Err(Refused::IntoItself));
    assert_eq!(v.unlink("", "hello.txt"), Err(Refused::Pinned));
    assert_eq!(v.unlink("nope", "x"), Err(Refused::NotFound));
    // Fifteen levels deep is the last directory that can still hold an
    // entry (MAX_DEPTH 16).
    let mut path = String::new();
    for i in 0..15 {
        v.mkdir(&path, &format!("d{i}")).unwrap();
        path = if path.is_empty() {
            format!("d{i}")
        } else {
            format!("{path}/d{i}")
        };
    }
    assert_eq!(v.mkdir(&path, "too_deep"), Err(Refused::TooDeep));
    v.create(&path, "leaf", b"x").unwrap();
    // A tree two levels tall can't move below level 14.
    assert_eq!(v.rename("c", "a", &path, "a"), Err(Refused::TooDeep));
}
