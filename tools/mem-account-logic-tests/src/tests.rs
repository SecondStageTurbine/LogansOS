// SPDX-License-Identifier: MPL-2.0
use super::kernel::{
    ChargeLedger, MEM_INFO_FLAG_OWN_BUDGET, MEM_INFO_V2_LEN, MemAccount, ObjCharge, QuotaExceeded,
    UNLIMITED, Underflow, budget_frames, encode_mem_info, encode_mem_info_v2,
};
use super::libneo;
use std::sync::Arc;

// ── the account ──────────────────────────────────────────────────────────

#[test]
fn charge_fits_then_refuses_at_the_limit() {
    let root = MemAccount::root();
    let a = MemAccount::child(&root, 8);
    assert_eq!(a.charge(5), Ok(()));
    assert_eq!(a.charge(3), Ok(()));
    assert_eq!(a.charge(1), Err(QuotaExceeded));
    assert_eq!((a.used(), root.used()), (8, 8));
}

#[test]
fn a_refused_charge_changes_nothing() {
    let root = MemAccount::root();
    let a = MemAccount::child(&root, 4);
    assert_eq!(a.charge(5), Err(QuotaExceeded));
    assert_eq!((a.used(), root.used()), (0, 0));
}

#[test]
fn the_parent_limit_bounds_its_children_together() {
    // Design §9 role 4: two 8-frame children under a 12-frame parent.
    let root = MemAccount::root();
    let browser = MemAccount::child(&root, 12);
    let c1 = MemAccount::child(&browser, 8);
    let c2 = MemAccount::child(&browser, 8);
    assert_eq!(c1.charge(8), Ok(()));
    assert_eq!(c2.charge(4), Ok(()));
    // c2 has room (4 of 8) but the parent is full: refused, rolled back.
    assert_eq!(c2.charge(1), Err(QuotaExceeded));
    assert_eq!(
        (c1.used(), c2.used(), browser.used(), root.used()),
        (8, 4, 12, 12)
    );
}

#[test]
fn rollback_releases_only_the_levels_it_charged() {
    let root = MemAccount::root();
    let top = MemAccount::child(&root, 3);
    let mid = MemAccount::child(&top, 10);
    let leaf = MemAccount::child(&mid, 10);
    assert_eq!(top.charge(2), Ok(())); // something else already uses top
    assert_eq!(leaf.charge(2), Err(QuotaExceeded)); // leaf, mid charged; top refuses
    assert_eq!(
        (leaf.used(), mid.used(), top.used(), root.used()),
        (0, 0, 2, 2)
    );
}

#[test]
fn unlimited_root_never_refuses_and_is_not_budgeted() {
    let root = MemAccount::root();
    assert_eq!(root.charge(1 << 40), Ok(()));
    assert!(!root.is_budgeted());
    assert!(MemAccount::child(&root, 1).is_budgeted());
    // A child of a budgeted account is budgeted even with no limit of its own.
    let b = MemAccount::child(&root, 5);
    assert!(MemAccount::child(&b, UNLIMITED).is_budgeted());
}

#[test]
fn charge_near_u64_max_does_not_wrap() {
    let root = MemAccount::root();
    assert_eq!(root.charge(UNLIMITED - 1), Ok(()));
    assert_eq!(root.charge(2), Err(QuotaExceeded));
    assert_eq!(root.used(), UNLIMITED - 1);
}

#[test]
fn release_more_than_used_is_refused_and_counted() {
    let root = MemAccount::root();
    let a = MemAccount::child(&root, 8);
    a.charge(2).unwrap();
    assert_eq!(a.release(3), Err(Underflow));
    assert_eq!((a.used(), root.used()), (2, 2));
    assert_eq!((a.underflows(), root.underflows()), (1, 1));
}

#[test]
fn an_account_keeps_its_parent_alive() {
    // PM Q3: the chain walk never reaches a freed account.
    let root = MemAccount::root();
    let child = {
        let parent = MemAccount::child(&root, 8);
        MemAccount::child(&parent, 4)
        // `parent` (the only other handle) drops here
    };
    assert_eq!(child.charge(4), Ok(()));
    assert_eq!(root.used(), 4);
    assert_eq!(child.release(4), Ok(()));
    assert_eq!(root.used(), 0);
}

// ── one case per charging path (PM Q4: no underflow, no double release) ──

fn budgeted() -> (Arc<MemAccount>, Arc<MemAccount>) {
    let root = MemAccount::root();
    let a = MemAccount::child(&root, 16);
    (root, a)
}

#[test]
fn path_map_success_then_drop() {
    let (root, a) = budgeted();
    let pmap = ChargeLedger::new(a.clone());
    pmap.charge(1).unwrap(); // PML4
    pmap.charge(3).unwrap(); // three region pages
    assert_eq!(pmap.release_all(), Ok(())); // Drop
    assert_eq!((pmap.held(), a.used(), root.used()), (0, 0, 0));
}

#[test]
fn path_failed_map_releases_its_charge() {
    // charge before allocate_frame; the allocator fails; release.
    let (root, a) = budgeted();
    let pmap = ChargeLedger::new(a.clone());
    pmap.charge(1).unwrap();
    assert_eq!(pmap.release(1), Ok(()));
    assert_eq!((pmap.held(), a.used(), root.used()), (0, 0, 0));
}

#[test]
fn path_unmap_range_then_drop_does_not_double_release() {
    let (root, a) = budgeted();
    let pmap = ChargeLedger::new(a.clone());
    pmap.charge(5).unwrap();
    for _ in 0..4 {
        pmap.release(1).unwrap(); // unmap_user_range, per freed frame
    }
    assert_eq!(pmap.release_all(), Ok(())); // Drop releases only the 1 left
    assert_eq!(pmap.release_all(), Ok(())); // and is idempotent
    assert_eq!((a.used(), root.used(), a.underflows()), (0, 0, 0));
}

#[test]
fn path_unmap_user_hands_the_frame_back() {
    let (_root, a) = budgeted();
    let pmap = ChargeLedger::new(a.clone());
    pmap.charge(2).unwrap(); // PML4 + a caller-supplied frame
    pmap.release(1).unwrap(); // unmap_user returns the frame to its caller
    assert_eq!(pmap.held(), 1);
    pmap.release_all().unwrap();
    assert_eq!(a.used(), 0);
}

#[test]
fn path_ledger_refuses_to_release_what_it_did_not_charge() {
    let (_root, a) = budgeted();
    let other = ChargeLedger::new(a.clone());
    other.charge(3).unwrap();
    let pmap = ChargeLedger::new(a.clone());
    pmap.charge(1).unwrap();
    // A bug releasing 2 through `pmap` must not eat `other`'s charge.
    assert_eq!(pmap.release(2), Err(Underflow));
    assert_eq!((pmap.held(), a.used()), (1, 4));
}

#[test]
fn path_failed_spawn_cp89() {
    // CP-89: the spawn charges the child's PML4 and image to the new account,
    // then fails before the child is registered. Dropping the Pmap (its
    // ledger) releases everything, back up the chain.
    let root = MemAccount::root();
    let spawner = MemAccount::child(&root, 64);
    spawner.charge(10).unwrap(); // the spawner's own frames
    {
        let child = MemAccount::child(&spawner, 4);
        let pmap = ChargeLedger::new(child.clone());
        pmap.charge(1).unwrap(); // PML4
        pmap.charge(3).unwrap(); // the image fills the budget
        assert_eq!(pmap.charge(1), Err(QuotaExceeded)); // the stack doesn't fit
        pmap.release_all().unwrap(); // the error path drops the Pmap
        assert_eq!(child.used(), 0);
    }
    assert_eq!((spawner.used(), root.used()), (10, 10));
}

#[test]
fn path_buffer_charged_to_creator_until_last_reference() {
    // An Atrium buffer holds its creator's ledger; a grant to the compositor
    // only clones the buffer Arc, so the charge stays until the last drop.
    let (root, a) = budgeted();
    let buffer = Arc::new(ChargeLedger::new(a.clone()));
    buffer.charge(8).unwrap();
    let compositor_copy = buffer.clone();
    drop(buffer); // the creator closes its handle
    assert_eq!(a.used(), 8);
    compositor_copy.release_all().unwrap(); // the buffer's Drop
    assert_eq!((a.used(), root.used()), (0, 0));
}

#[test]
fn path_dropped_ledger_releases_what_it_holds() {
    // An owner's early return (the buffer's allocator failing after the
    // charge) drops the ledger: the charge must not leak.
    let (root, a) = budgeted();
    {
        let buffer = ChargeLedger::new(a.clone());
        buffer.charge(6).unwrap();
    }
    assert_eq!((a.used(), root.used()), (0, 0));
    // After an explicit release_all, the drop releases nothing more.
    {
        let pmap = ChargeLedger::new(a.clone());
        pmap.charge(2).unwrap();
        pmap.release_all().unwrap();
    }
    assert_eq!((a.used(), a.underflows()), (0, 0));
}

// ── concurrency: two CPUs charging one chain ─────────────────────────────

#[test]
fn hammer_never_exceeds_a_limit_and_returns_to_zero() {
    let root = MemAccount::root();
    let browser = MemAccount::child(&root, 50);
    let kids: Vec<_> = (0..4).map(|_| MemAccount::child(&browser, 20)).collect();
    std::thread::scope(|s| {
        for k in &kids {
            let k = k.clone();
            let browser = browser.clone();
            s.spawn(move || {
                let mut held = 0u64;
                for i in 0..20_000u64 {
                    if i % 3 == 2 && held > 0 {
                        k.release(1).unwrap();
                        held -= 1;
                    } else if k.charge(1).is_ok() {
                        held += 1;
                    }
                    assert!(k.used() <= 20);
                    assert!(browser.used() <= 50);
                }
                k.release(held).unwrap();
            });
        }
    });
    assert_eq!((browser.used(), root.used()), (0, 0));
    assert_eq!(root.underflows(), 0);
}

// ── budget rounding ──────────────────────────────────────────────────────

#[test]
fn budget_bytes_round_up_to_frames() {
    assert_eq!(budget_frames(0), None);
    assert_eq!(budget_frames(1), Some(1));
    assert_eq!(budget_frames(4096), Some(1));
    assert_eq!(budget_frames(4097), Some(2));
    assert_eq!(budget_frames(8 << 20), Some(2048));
    assert_eq!(budget_frames(u64::MAX), Some(u64::MAX.div_ceil(4096)));
}

// ── SYS_MEM_INFO byte oracle (hand-laid, not a round trip) ───────────────

/// A budgeted process: own budget, 300 frames used of 2048, 30000 in use
/// system-wide.
const BUDGETED: [u8; 32] = [
    0x01, 0x00, 0x00, 0x00, // version 1
    0x01, 0x00, 0x00, 0x00, // flags: own budget
    0x2c, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // used 300
    0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // limit 2048
    0x30, 0x75, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // global used 30000
];

/// init: no budget, 5 frames used, limit unlimited, global used
/// 0x0102030405060708.
const ROOT: [u8; 32] = [
    0x01, 0x00, 0x00, 0x00, // version 1
    0x00, 0x00, 0x00, 0x00, // flags: none
    0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // used 5
    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, // limit: unlimited
    0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, // global used
];

#[test]
fn kernel_encoder_matches_hand_laid_bytes() {
    assert_eq!(
        encode_mem_info(MEM_INFO_FLAG_OWN_BUDGET, 300, 2048, 30000),
        BUDGETED
    );
    assert_eq!(
        encode_mem_info(0, 5, UNLIMITED, 0x0102_0304_0506_0708),
        ROOT
    );
}

#[test]
fn libneo_decoder_matches_hand_laid_bytes() {
    assert_eq!(
        libneo::decode(&BUDGETED),
        Some(libneo::MemInfo {
            flags: libneo::FLAG_OWN_BUDGET,
            used_frames: 300,
            limit_frames: 2048,
            global_used_frames: 30000,
        })
    );
    let root = libneo::decode(&ROOT).unwrap();
    assert_eq!((root.flags, root.used_frames), (0, 5));
    assert_eq!(root.limit_frames, libneo::UNLIMITED);
    assert_eq!(root.global_used_frames, 0x0102_0304_0506_0708);
}

#[test]
fn decoder_refuses_another_version() {
    let mut rec = BUDGETED;
    rec[0] = 2;
    assert_eq!(libneo::decode(&rec), None);
}

#[test]
fn both_sides_agree_on_the_layout_constants() {
    assert_eq!(super::kernel::MEM_INFO_V1_LEN, libneo::RECORD_LEN);
    assert_eq!(MEM_INFO_FLAG_OWN_BUDGET, libneo::FLAG_OWN_BUDGET);
    assert_eq!(UNLIMITED, libneo::UNLIMITED);
}

// ── OT-61 C2.2b: the object counter ─────────────────────────────────────

#[test]
fn obj_charge_fits_then_refuses_at_the_limit_and_rolls_back() {
    let root = MemAccount::root_with_obj_limit(1000);
    let browser = MemAccount::child_with_obj_limit(&root, UNLIMITED, 600);
    let tab = MemAccount::child_with_obj_limit(&browser, UNLIMITED, 400);
    assert_eq!(tab.charge_obj(400), Ok(()));
    assert_eq!(tab.charge_obj(1), Err(QuotaExceeded)); // its own limit
    assert_eq!(
        (tab.obj_used(), browser.obj_used(), root.obj_used()),
        (400, 400, 400)
    );
    let other = MemAccount::child_with_obj_limit(&browser, UNLIMITED, 400);
    // room in `other` (0 of 400), but the browser has only 200 left
    assert_eq!(other.charge_obj(300), Err(QuotaExceeded));
    assert_eq!((other.obj_used(), browser.obj_used()), (0, 400));
    assert_eq!(other.charge_obj(200), Ok(()));
    assert_eq!(root.obj_used(), 600);
}

#[test]
fn obj_and_frame_counters_are_independent() {
    let root = MemAccount::root_with_obj_limit(UNLIMITED);
    let a = MemAccount::child_with_obj_limit(&root, 4, 100);
    assert_eq!(a.charge(4), Ok(()));
    assert_eq!(a.charge(1), Err(QuotaExceeded)); // frames full
    assert_eq!(a.charge_obj(100), Ok(())); // objects still fit
    assert_eq!(a.charge_obj(1), Err(QuotaExceeded));
    assert_eq!(a.charge(0), Ok(()));
    assert_eq!((a.used(), a.obj_used()), (4, 100));
    assert!(a.is_budgeted());
    // a frame-only child of an object-only parent is budgeted too
    let b = MemAccount::child_with_obj_limit(&a, UNLIMITED, UNLIMITED);
    assert!(b.is_budgeted());
}

#[test]
fn obj_charge_releases_exactly_once_when_dropped() {
    let root = MemAccount::root_with_obj_limit(UNLIMITED);
    let a = MemAccount::child_with_obj_limit(&root, UNLIMITED, 64);
    let c = ObjCharge::new(&a, 48).unwrap();
    assert_eq!(c.bytes(), 48);
    assert!(ObjCharge::new(&a, 17).is_err());
    assert_eq!(a.obj_used(), 48);
    drop(c);
    assert_eq!(
        (a.obj_used(), root.obj_used(), root.underflows()),
        (0, 0, 0)
    );
}

#[test]
fn obj_release_more_than_used_is_refused_and_counted() {
    let root = MemAccount::root_with_obj_limit(UNLIMITED);
    let a = MemAccount::child_with_obj_limit(&root, UNLIMITED, 10);
    assert_eq!(a.charge_obj(3), Ok(()));
    assert_eq!(a.release_obj(4), Err(Underflow));
    assert_eq!(a.obj_used(), 3);
    assert!(a.underflows() >= 1);
}

#[test]
fn obj_hammer_never_exceeds_a_limit_and_returns_to_zero() {
    let root = MemAccount::root_with_obj_limit(UNLIMITED);
    let browser = MemAccount::child_with_obj_limit(&root, UNLIMITED, 5_000);
    let kids: Vec<_> = (0..4)
        .map(|_| MemAccount::child_with_obj_limit(&browser, UNLIMITED, 2_000))
        .collect();
    std::thread::scope(|s| {
        for k in &kids {
            let k = k.clone();
            let browser = browser.clone();
            s.spawn(move || {
                let mut held = Vec::new();
                for i in 0..20_000u64 {
                    if i % 3 == 2 && !held.is_empty() {
                        held.pop();
                    } else if let Ok(c) = ObjCharge::new(&k, 100 + i % 50) {
                        held.push(c);
                    }
                    assert!(k.obj_used() <= 2_000);
                    assert!(browser.obj_used() <= 5_000);
                }
            });
        }
    });
    assert_eq!(
        (browser.obj_used(), root.obj_used(), root.underflows()),
        (0, 0, 0)
    );
}

// ── SYS_MEM_INFO v2 byte oracle ──────────────────────────────────────────

/// A budgeted process: own budget, 300 of 2048 frames, 30000 frames in use
/// system-wide, 0x1234 object bytes used of 65536.
const BUDGETED_V2: [u8; 48] = [
    0x02, 0x00, 0x00, 0x00, // version 2
    0x01, 0x00, 0x00, 0x00, // flags: own budget
    0x2c, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // used 300
    0x00, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // limit 2048
    0x30, 0x75, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // global used 30000
    0x34, 0x12, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // object bytes used
    0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, // object byte limit 65536
];

#[test]
fn v2_kernel_encoder_matches_hand_laid_bytes() {
    assert_eq!(MEM_INFO_V2_LEN, 48);
    assert_eq!(
        encode_mem_info_v2(MEM_INFO_FLAG_OWN_BUDGET, 300, 2048, 30000, 0x1234, 65536),
        BUDGETED_V2
    );
}

#[test]
fn v2_libneo_decoder_matches_hand_laid_bytes() {
    assert_eq!(libneo::RECORD_V2_LEN, MEM_INFO_V2_LEN);
    let d = libneo::decode_v2(&BUDGETED_V2).unwrap();
    assert_eq!(
        d.frames,
        libneo::MemInfo {
            flags: libneo::FLAG_OWN_BUDGET,
            used_frames: 300,
            limit_frames: 2048,
            global_used_frames: 30000,
        }
    );
    assert_eq!((d.obj_used_bytes, d.obj_limit_bytes), (0x1234, 65536));
}

#[test]
fn each_decoder_refuses_the_other_version() {
    let mut v1 = [0u8; 32];
    v1.copy_from_slice(&BUDGETED_V2[..32]);
    assert_eq!(libneo::decode(&v1), None); // version 2 in a v1 buffer
    let mut v2 = BUDGETED_V2;
    v2[0] = 1;
    assert_eq!(libneo::decode_v2(&v2), None);
}
