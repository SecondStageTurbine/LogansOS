// SPDX-License-Identifier: MPL-2.0
//! OT-87 R4.2: the ESP templates, their slot maps and the bundle pin.
//!
//! Each template is a 512 MiB image, so the tests that build one take
//! [`HEAVY`] and hold one image at a time.

use std::sync::Mutex;

use crate::esp_template::{self, ESP_BYTES, LIMINE_CONF_BYTES, SLOT_FILES, SLOTS};
use crate::install_bundle;
use crate::install_bundle_logic as logic;

static HEAVY: Mutex<()> = Mutex::new(());

/// A stand-in for Limine's `BOOTX64.EFI`: 245,760 bytes (v8.6.0's size),
/// none of them zero.
fn bootx64() -> Vec<u8> {
    (0..245_760u32).map(|i| (i % 251) as u8 + 1).collect()
}

fn le32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}

/// Every slot file is where its row says: the extent holds the file's bytes,
/// and the short directory entry the row names carries its size and first
/// cluster.
#[test]
fn templates_hold_their_slot_files_where_the_map_says() {
    let _heavy = HEAVY.lock().unwrap_or_else(|e| e.into_inner());
    for sector in [512u32, 4096] {
        let (image, map) = esp_template::build(sector, &bootx64()).unwrap();
        assert_eq!(image.len() as u64, ESP_BYTES);
        assert_eq!(map.sector_size, sector);
        let paths: Vec<_> = map.entries.iter().map(|e| e.path.clone()).collect();
        let want: Vec<_> = esp_template::slot_paths()
            .into_iter()
            .map(|p| p.0)
            .collect();
        assert_eq!(paths, want);
        let g = esp_template::geometry(&image).unwrap();
        // FAT32 by definition (FAT spec §3.5): at least 65,525 clusters, or a
        // reader takes it for FAT16.
        assert!(g.clusters >= 65_525, "{} clusters", g.clusters);
        let s = sector as usize;
        for (entry, (_, size)) in map.entries.iter().zip(esp_template::slot_paths()) {
            assert_eq!(entry.sectors * sector as u64, size.div_ceil(4096) * 4096);
            let dirent = entry.dirent_sector as usize * s + entry.dirent_offset as usize;
            assert_eq!(le32(&image, dirent + 28) as u64, size, "{}", entry.path);
            let cluster = (u16::from_le_bytes([image[dirent + 20], image[dirent + 21]]) as u64)
                << 16
                | u16::from_le_bytes([image[dirent + 26], image[dirent + 27]]) as u64;
            assert_eq!(
                g.data_start + (cluster - 2) * g.sectors_per_cluster,
                entry.first_sector,
                "{}",
                entry.path
            );
            let extent = &image[entry.first_sector as usize * s..][..size as usize];
            if entry.path == "limine.conf" {
                let conf = esp_template::padded_limine_conf(&esp_template::installed_limine_conf())
                    .unwrap();
                assert_eq!(extent, &conf[..]);
            } else {
                assert!(
                    extent.iter().all(|&b| b == 0),
                    "{} is not empty",
                    entry.path
                );
            }
        }
        // The map passes the installer's own checks (no overlaps, directory
        // entries outside every extent), and round-trips.
        let stored = logic::encode_slot_map(&map).unwrap();
        assert_eq!(logic::slot_map(&stored).unwrap(), map);
        // BOOTX64.EFI is in the image whole.
        let efi = bootx64();
        assert!(image.windows(64).any(|w| w == &efi[..64]));
    }
}

/// The extents as built today, pinned: a change to the vendored `fatfs` or to
/// the slot table shows here (and in the bundle's BLAKE3) before it reaches a
/// disk. `(path, first sector, sectors, dirent sector, dirent offset)`.
#[test]
fn slot_extents_are_pinned() {
    let _heavy = HEAVY.lock().unwrap_or_else(|e| e.into_inner());
    let row = |map: &logic::SlotMap, i: usize| {
        let e = &map.entries[i];
        (
            e.path.clone(),
            e.first_sector,
            e.sectors,
            e.dirent_sector,
            e.dirent_offset,
        )
    };
    let (_, map) = esp_template::build(512, &bootx64()).unwrap();
    assert_eq!(row(&map, 0), ("limine.conf".into(), 2582, 128, 2054, 192));
    assert_eq!(
        row(&map, 1),
        ("slot/A/kernel".into(), 2710, 147_456, 2086, 96)
    );
    assert_eq!(
        row(&map, 6),
        ("slot/B/kernel".into(), 510_614, 147_456, 2094, 96)
    );
    let (_, map) = esp_template::build(4096, &bootx64()).unwrap();
    assert_eq!(row(&map, 0), ("limine.conf".into(), 330, 16, 264, 192));
    assert_eq!(row(&map, 1), ("slot/A/kernel".into(), 346, 18_432, 268, 96));
    assert_eq!(
        row(&map, 10),
        ("slot/B/neo-arthur".into(), 116_058, 11_264, 269, 352)
    );
}

/// The template build refuses an image whose slot file isn't one contiguous
/// chain, whose FAT copies disagree, or whose file is the wrong size.
#[test]
fn a_non_contiguous_chain_fails_the_build() {
    let _heavy = HEAVY.lock().unwrap_or_else(|e| e.into_inner());
    let (mut image, map) = esp_template::build(512, &bootx64()).unwrap();
    assert!(esp_template::read_back(&image).is_ok());
    let g = esp_template::geometry(&image).unwrap();
    let kernel = &map.entries[1];
    let c = ((kernel.first_sector - g.data_start) / g.sectors_per_cluster + 2) as usize;
    let fat = |copy: u64| ((g.fat_start + copy * g.fat_sectors) * g.sector) as usize;
    let set = |image: &mut Vec<u8>, cluster: usize, next: u32| {
        for copy in 0..g.fats {
            let at = fat(copy) + cluster * 4;
            image[at..at + 4].copy_from_slice(&next.to_le_bytes());
        }
    };
    let saved = image[fat(0)..fat(0) + (c + 4) * 4].to_vec();
    let saved1 = image[fat(1)..fat(1) + (c + 4) * 4].to_vec();
    // c → c+2 → c+1 → c+3: the same clusters, out of order.
    set(&mut image, c, (c + 2) as u32);
    set(&mut image, c + 2, (c + 1) as u32);
    set(&mut image, c + 1, (c + 3) as u32);
    let err = esp_template::read_back(&image).unwrap_err();
    assert!(err.contains("slot/A/kernel is not contiguous"), "{err}");
    image[fat(0)..fat(0) + saved.len()].copy_from_slice(&saved);
    image[fat(1)..fat(1) + saved1.len()].copy_from_slice(&saved1);
    assert!(esp_template::read_back(&image).is_ok());
    // FAT copy 1 differs from copy 0 at the kernel's first cluster.
    let at = fat(1) + c * 4;
    image[at] ^= 1;
    let err = esp_template::read_back(&image).unwrap_err();
    assert!(err.contains("FAT copy 1 differs"), "{err}");
    image[at] ^= 1;
    // The kernel's directory entry says one byte less.
    let dirent = (kernel.dirent_sector * g.sector) as usize + kernel.dirent_offset as usize;
    let size = le32(&image, dirent + 28);
    image[dirent + 28..dirent + 32].copy_from_slice(&(size - 1).to_le_bytes());
    let err = esp_template::read_back(&image).unwrap_err();
    assert!(err.contains("slot/A/kernel is"), "{err}");
}

/// The slot table's budget (THINK §7.2): both slots, `limine.conf` and
/// Limine fit the ESP's data area, and the measured deployment fits each
/// file with room to spare.
#[test]
fn the_slot_files_fit_the_esp_and_the_measured_deployment() {
    let per_slot: u64 = SLOT_FILES.iter().map(|f| f.1).sum();
    assert_eq!(per_slot, 248 << 20);
    let used = SLOTS.len() as u64 * per_slot + LIMINE_CONF_BYTES + 245_760;
    assert!(used + (8 << 20) < ESP_BYTES, "{used}");
    // §7.2, image A's deployment on origin/main 0b39255b.
    let measured = [
        ("kernel", 39_589_264u64),
        ("neo-init", 23_221_552),
        ("neo-worker", 23_206_992),
        ("neo-shell", 23_322_560),
        ("neo-arthur", 24_403_664),
    ];
    for ((name, max), (m_name, m)) in SLOT_FILES.iter().zip(measured) {
        assert_eq!(*name, m_name);
        assert!(
            m * 3 / 2 < *max,
            "{name}: {m} leaves less than half again in {max}"
        );
    }
    let conf = esp_template::installed_limine_conf();
    assert!((conf.len() as u64) < LIMINE_CONF_BYTES / 16);
    for (name, _) in SLOT_FILES {
        assert!(conf.contains(&format!("boot():/slot/A/{name}\n")), "{name}");
    }
    assert!(!conf.contains("/slot/B/"));
}

/// The bundle is reproducible, carries its five parts, and a staged bundle
/// that doesn't hash to the pin is refused.
#[test]
fn the_bundle_is_reproducible_and_pinned() {
    let _heavy = HEAVY.lock().unwrap_or_else(|e| e.into_inner());
    let mut seed = vec![0u8; 8 << 20];
    seed[0..8].copy_from_slice(b"NEOFSv1\0");
    seed[(4 << 20) + 17] = 0x42;
    let a = install_bundle::assemble(&bootx64(), &seed).unwrap();
    let b = install_bundle::assemble(&bootx64(), &seed).unwrap();
    assert_eq!(a, b);
    let parts = logic::installer_manifest(&a).unwrap();
    assert_eq!(
        parts.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(),
        logic::PART_NAMES
    );
    assert_eq!(parts[0].expanded, ESP_BYTES);
    assert_eq!(parts[4].expanded, seed.len() as u64);
    // Small: the templates are sparse.
    assert!(a.len() < 4 << 20, "{}", a.len());
    let pin = crate::hex_string(blake3::hash(&a).as_bytes());
    assert!(install_bundle::check_staged(&a, &pin).is_ok());
    assert!(install_bundle::check_staged(&a, &pin.to_uppercase()).is_ok());
    let mut other = a.clone();
    let last = other.len() - 1;
    other[last] ^= 1;
    let err = install_bundle::check_staged(&other, &pin).unwrap_err();
    assert!(err.contains("does not match the kernel's pin"), "{err}");
    // A different seed is a different bundle.
    seed[1] = 1;
    let c = install_bundle::assemble(&bootx64(), &seed).unwrap();
    assert!(install_bundle::check_staged(&c, &pin).is_err());
}

/// R4.3: a deployment file larger than its slot fails the image build
/// (§7.2), at exactly one byte over.
#[test]
fn a_file_one_byte_over_its_slot_fails_the_build() {
    assert!(install_bundle::slot_fits("kernel", 72 << 20).is_ok());
    assert!(install_bundle::slot_fits("kernel", (72 << 20) + 1).is_err());
    for name in ["neo-init", "neo-worker", "neo-shell", "neo-arthur"] {
        assert!(install_bundle::slot_fits(name, 44 << 20).is_ok());
        assert!(install_bundle::slot_fits(name, (44 << 20) + 1).is_err());
    }
    assert!(install_bundle::slot_fits("neo-other", 1).is_err());
}
