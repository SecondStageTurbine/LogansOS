// SPDX-License-Identifier: MPL-2.0
//! Host KATs for `kernel/src/mm/module_reclaim_logic.rs` (B1a, Chad C2):
//! which pages of a consumed boot module may be admitted to the frame
//! allocator, and how an admitted zone is tiled into buddy blocks. Also
//! `kernel/src/boot_model_logic.rs` (B1b): the boot-module model pins.
//!
//! The real module is mounted through `#[path]`, so these tests exercise the
//! exact code the kernel runs.

#![allow(dead_code)]

#[path = "../../../kernel/src/mm/module_reclaim_logic.rs"]
mod module_reclaim_logic;

#[path = "../../../kernel/src/boot_model_logic.rs"]
mod boot_model_logic;

#[path = "../../../kernel/src/mm/inference_stack_logic.rs"]
mod inference_stack_logic;

#[cfg(test)]
mod tests {
    use super::module_reclaim_logic::{
        MEMMAP_EXECUTABLE_AND_MODULES as MODS, MapEntry, PAGE, PlanRefusal, Span, blocks,
        plan_module_reclaim, zone_base,
    };

    const USABLE: u64 = 0;
    const RESERVED: u64 = 1;
    const RECLAIMABLE: u64 = 5;

    const fn entry(base: u64, length: u64, kind: u64) -> MapEntry {
        MapEntry { base, length, kind }
    }

    /// The real Qwen2.5-0.5B `.neom`: 485,489,152 bytes, 2,560 past a page.
    const NEOM_05B: u64 = 485_489_152;
    const EXEC: Span = Span::new(0x0010_0000, 0x0050_0000);

    #[test]
    fn real_05b_length_keeps_its_unaligned_tail_page() {
        assert_eq!(NEOM_05B % PAGE, 2560);
        let base = 0x4000_0000;
        let module = Span::new(base, base + NEOM_05B);
        let memmap = [entry(base, NEOM_05B, MODS)];
        let plan = plan_module_reclaim(module, &memmap, EXEC, &[]).unwrap();
        assert_eq!(plan, Span::new(base, base + NEOM_05B - 2560));
        assert_eq!(plan.pages(), NEOM_05B / PAGE);
    }

    #[test]
    fn unaligned_head_is_rounded_inward() {
        let module = Span::new(0x4000_0123, 0x4000_9000);
        let memmap = [entry(0x4000_0000, 0x9000, MODS)];
        let plan = plan_module_reclaim(module, &memmap, EXEC, &[]).unwrap();
        assert_eq!(plan, Span::new(0x4000_1000, 0x4000_9000));
    }

    #[test]
    fn sub_page_module_admits_nothing() {
        let module = Span::new(0x4000_0010, 0x4000_0ff0);
        let memmap = [entry(0x4000_0000, PAGE, MODS)];
        assert_eq!(
            plan_module_reclaim(module, &memmap, EXEC, &[]),
            Err(PlanRefusal::NoWholePage)
        );
        assert_eq!(
            plan_module_reclaim(Span::new(5, 5), &memmap, EXEC, &[]),
            Err(PlanRefusal::EmptyModule)
        );
    }

    #[test]
    fn overlapping_and_merged_type6_entries_cover() {
        let module = Span::new(0x4000_0000, 0x4001_0000);
        // Three type-6 entries that overlap each other and only together
        // cover the module (Limine may describe type 6 this way).
        let memmap = [
            entry(0x3fff_f000, 0x6000, MODS),
            entry(0x4000_4000, 0x8000, MODS),
            entry(0x4000_b000, 0x8000, MODS),
        ];
        assert_eq!(
            plan_module_reclaim(module, &memmap, EXEC, &[]),
            Ok(Span::new(0x4000_0000, 0x4001_0000))
        );
    }

    #[test]
    fn a_gap_in_type6_coverage_is_refused() {
        let module = Span::new(0x4000_0000, 0x4001_0000);
        let memmap = [
            entry(0x4000_0000, 0x4000, MODS),
            // 0x4000_4000..0x4000_5000 is described by nothing.
            entry(0x4000_5000, 0xb000, MODS),
        ];
        assert_eq!(
            plan_module_reclaim(module, &memmap, EXEC, &[]),
            Err(PlanRefusal::NotCovered)
        );
    }

    #[test]
    fn any_foreign_type_overlap_is_refused() {
        let module = Span::new(0x4000_0000, 0x4001_0000);
        for kind in [USABLE, RESERVED, RECLAIMABLE] {
            let memmap = [
                entry(0x4000_0000, 0x1_0000, MODS),
                entry(0x4000_f000, 0x1000, kind),
            ];
            assert_eq!(
                plan_module_reclaim(module, &memmap, EXEC, &[]),
                Err(PlanRefusal::ForeignTypeOverlap),
                "type {kind}"
            );
        }
        // A wrong-typed description of the whole module (the forged-plan control).
        let memmap = [entry(0x4000_0000, 0x1_0000, RESERVED)];
        assert_eq!(
            plan_module_reclaim(module, &memmap, EXEC, &[]),
            Err(PlanRefusal::ForeignTypeOverlap)
        );
        // No description at all.
        assert_eq!(
            plan_module_reclaim(module, &[], EXEC, &[]),
            Err(PlanRefusal::NotCovered)
        );
    }

    #[test]
    fn a_page_shared_with_the_executable_is_refused() {
        // The executable ends mid-page inside the module's first whole page.
        let exec = Span::new(0x3ff0_0000, 0x4000_0800);
        let module = Span::new(0x4000_0000, 0x4001_0000);
        let memmap = [entry(0x3ff0_0000, 0x11_0000, MODS)];
        assert_eq!(
            plan_module_reclaim(module, &memmap, exec, &[]),
            Err(PlanRefusal::ExecutableOverlap)
        );
        // Ending exactly at the module's first page is fine.
        let exec = Span::new(0x3ff0_0000, 0x4000_0000);
        assert!(plan_module_reclaim(module, &memmap, exec, &[]).is_ok());
    }

    #[test]
    fn an_edge_page_shared_with_another_module_is_retained() {
        // The neighbour ends 0x100 into the page the module starts in: that
        // page is not one of the module's whole pages, so it is retained and
        // the rest is admitted.
        let neighbour = Span::new(0x3fff_0000, 0x4000_0100);
        let module = Span::new(0x4000_0100, 0x4001_0000);
        let memmap = [entry(0x3fff_0000, 0x2_0000, MODS)];
        assert_eq!(
            plan_module_reclaim(module, &memmap, EXEC, &[neighbour]),
            Ok(Span::new(0x4000_1000, 0x4001_0000))
        );
    }

    #[test]
    fn an_interior_overlap_with_another_module_is_refused() {
        let module = Span::new(0x4000_0000, 0x4001_0000);
        let intruder = Span::new(0x4000_8000, 0x4000_8001);
        let memmap = [entry(0x4000_0000, 0x1_0000, MODS)];
        assert_eq!(
            plan_module_reclaim(module, &memmap, EXEC, &[intruder]),
            Err(PlanRefusal::ModuleOverlap)
        );
        // Empty neighbours are ignored.
        assert!(plan_module_reclaim(module, &memmap, EXEC, &[Span::new(9, 9)]).is_ok());
    }

    #[test]
    fn fragmented_memmap_with_interleaved_types() {
        // The module sits between usable and reserved neighbours, described
        // by one type-6 entry that exactly matches it.
        let module = Span::new(0x8000_0000, 0x8040_0000);
        let memmap = [
            entry(0x0000_0000, 0x8000_0000, USABLE),
            entry(0x8000_0000, 0x40_0000, MODS),
            entry(0x8040_0000, 0x1000, RESERVED),
            entry(0x8040_1000, 0x7fbf_f000, USABLE),
        ];
        assert_eq!(plan_module_reclaim(module, &memmap, EXEC, &[]), Ok(module));
    }

    #[test]
    fn overflow_at_the_top_of_the_address_space_is_refused() {
        let top = u64::MAX - 0x800;
        let module = Span::new(top, u64::MAX);
        let memmap = [entry(top, 0x800, MODS)];
        assert_eq!(
            plan_module_reclaim(module, &memmap, EXEC, &[]),
            Err(PlanRefusal::Overflow)
        );
        let module = Span::new(0x4000_0000, 0x4001_0000);
        let memmap = [entry(u64::MAX - 0x10, 0x100, MODS)];
        assert_eq!(
            plan_module_reclaim(module, &memmap, EXEC, &[]),
            Err(PlanRefusal::Overflow)
        );
    }

    #[test]
    fn blocks_tile_the_span_exactly_and_are_naturally_aligned() {
        for (start, end, max_order) in [
            (0x4000_1000u64, 0x4001_0000u64, 14usize),
            (0x4000_0000, 0x4000_0000 + 485_486_592, 14),
            (0x1234_5000, 0x1234_6000, 14),
            (0x7_f000, 0x200_3000, 3),
        ] {
            let span = Span::new(start, end);
            let mut cursor = start;
            let mut pages = 0;
            for (addr, order) in blocks(span, max_order) {
                assert_eq!(addr, cursor, "contiguous");
                assert!(order <= max_order);
                let size = PAGE << order;
                assert_eq!(addr % size, 0, "naturally aligned");
                cursor += size;
                pages += 1u64 << order;
            }
            assert_eq!(cursor, end, "covers to the end, never past it");
            assert_eq!(pages, span.pages());
        }
        assert_eq!(blocks(Span::new(0x1000, 0x1000), 14).count(), 0);
    }

    #[test]
    fn zone_base_is_the_max_order_block_at_or_below_start() {
        let block = PAGE << 14;
        assert_eq!(zone_base(0x4000_1000, 14), 0x4000_0000);
        assert_eq!(zone_base(block * 3, 14), block * 3);
        assert_eq!(zone_base(block * 3 + PAGE, 14) % block, 0);
        assert!(zone_base(0x4000_1000, 14) <= 0x4000_1000);
    }
}

#[cfg(test)]
mod pin_tests {
    use super::boot_model_logic::{
        BOOT_MODEL_DIR, FIXTURE_05B, FIXTURE_05B_VOCAB, FIXTURE_05B_VOCB, KAT_INVENTORY,
        PRODUCT_PINS, PRODUCT_VOCAB_PINS, QWEN25_05B, QWEN25_05B_VOCAB, QWEN25_05B_VOCB, TEST_PINS,
        TEST_VOCAB_PINS, VOCAB_DIR, Verdict, expected, hex_digest, hex32, is_pinned, module_suffix,
        verdict,
    };

    #[test]
    fn vocab_pairs_its_model_only() {
        use super::boot_model_logic::vocab_pairs_model;
        let m = "/models/model-qwen2.5-0.5b.neom";
        assert!(vocab_pairs_model(m, "/vocab/model-qwen2.5-0.5b.vocab"));
        // The NEOVOCB1 form pairs too; no other extension does.
        assert!(vocab_pairs_model(m, "/vocab/model-qwen2.5-0.5b.vocb"));
        assert!(!vocab_pairs_model(m, "/vocab/model-qwen2.5-0.5b.vocbx"));
        assert!(!vocab_pairs_model(m, "/vocab/model-qwen2.5-0.5b.voc"));
        assert!(!vocab_pairs_model(m, "/vocab/model-qwen2.5-1.5b.vocab"));
        assert!(!vocab_pairs_model(m, "/vocab/model-qwen2.5-0.5b.neom"));
        assert!(!vocab_pairs_model(
            "/models/kv-persist-kat.neom",
            "/vocab/model-qwen2.5-0.5b.vocab"
        ));
        assert!(!vocab_pairs_model("/models/.neom", "/vocab/.vocab"));
        assert!(!vocab_pairs_model("/other/x.neom", "/vocab/x.vocab"));
    }

    /// OT-87 R2: a NeoFS file is held to the pin with its file name; every
    /// other path (unpinned, nested, outside `/neofs/`) has no pin.
    #[test]
    fn neofs_paths_map_to_their_pins() {
        use super::boot_model_logic::{NeofsPin, neofs_name, neofs_pin};
        let pin = |p| neofs_pin(PRODUCT_PINS, PRODUCT_VOCAB_PINS, p);
        assert_eq!(
            pin("/neofs/model-qwen2.5-0.5b.neom"),
            Some(NeofsPin::Model("/models/model-qwen2.5-0.5b.neom"))
        );
        assert_eq!(
            pin("/neofs/model-qwen2.5-0.5b.vocb"),
            Some(NeofsPin::Vocab("/vocab/model-qwen2.5-0.5b.vocb"))
        );
        assert_eq!(
            pin("/neofs/model-qwen2.5-0.5b.vocab"),
            Some(NeofsPin::Vocab("/vocab/model-qwen2.5-0.5b.vocab"))
        );
        // Unpinned names keep today's behaviour.
        assert_eq!(pin("/neofs/model-qwen2.5-1.5b.neom"), None);
        assert_eq!(pin("/neofs/model.neom"), None);
        // The test-only inventory model is not a product pin.
        assert_eq!(pin("/neofs/kv-persist-kat.neom"), None);
        assert_eq!(
            neofs_pin(TEST_PINS, TEST_VOCAB_PINS, "/neofs/kv-persist-kat.neom"),
            Some(NeofsPin::Model("/models/kv-persist-kat.neom"))
        );
        // Only a direct child of /neofs/ is pinned.
        assert_eq!(pin("/neofs/models/model-qwen2.5-0.5b.neom"), None);
        assert_eq!(pin("/neofs/"), None);
        assert_eq!(pin("/boot/models/model-qwen2.5-0.5b.neom"), None);
        assert_eq!(pin("/neofsx/model-qwen2.5-0.5b.neom"), None);
        assert_eq!(pin("neofs/model-qwen2.5-0.5b.neom"), None);
        // The name a pin's module has on NeoFS.
        assert_eq!(
            neofs_name("/models/model-qwen2.5-0.5b.neom"),
            Some("model-qwen2.5-0.5b.neom")
        );
        assert_eq!(
            neofs_name("/vocab/model-qwen2.5-0.5b.vocb"),
            Some("model-qwen2.5-0.5b.vocb")
        );
        assert_eq!(neofs_name("/models/"), None);
        assert_eq!(neofs_name("/models/a/b"), None);
        assert_eq!(neofs_name("/firmware/x"), None);
    }

    /// Byte oracle: the first and last bytes of the 0.5B pin, laid out by hand
    /// from the digest recorded in `docs/OPEN-THREADS.md` (OT-73), independent
    /// of `hex32`.
    #[test]
    fn real_05b_pin_bytes_are_the_recorded_digest() {
        let pin = QWEN25_05B.blake3;
        assert_eq!(&pin[..4], &[0xa3, 0x2e, 0x4a, 0x1b]);
        assert_eq!(&pin[28..], &[0x7e, 0x1c, 0xca, 0xfe]);
        assert_eq!(
            &hex_digest(&pin),
            b"a32e4a1ba94a850724c15d840f3ae2c44902d9f1f34d8069bac5fcb37e1ccafe"
        );
        assert_eq!(QWEN25_05B.module, "/models/model-qwen2.5-0.5b.neom");
    }

    #[test]
    fn hex_round_trips() {
        let mut bytes = [0u8; 32];
        for (i, b) in bytes.iter_mut().enumerate() {
            *b = (i as u8).wrapping_mul(37) ^ 0xa5;
        }
        assert_eq!(hex32(&hex_digest(&bytes)), bytes);
    }

    /// The fixture pin is what `neomc fixtures` writes today, so the hermetic
    /// scenario cannot drift into a "damaged" verdict without this failing first.
    #[test]
    fn fixture_pin_matches_the_generated_fixture() {
        let dir = std::env::temp_dir().join(format!("b1b-pin-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        neomc::fixtures::generate_all(&dir).unwrap();
        let bytes = std::fs::read(dir.join("valid_embedding.neom")).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(bytes.len(), 4144);
        assert_eq!(blake3::hash(&bytes).as_bytes(), &FIXTURE_05B.blake3);
    }

    #[test]
    fn product_kernels_accept_only_the_real_model() {
        assert_eq!(PRODUCT_PINS, &[QWEN25_05B]);
        let m = QWEN25_05B.module;
        assert_eq!(verdict(PRODUCT_PINS, m, &QWEN25_05B.blake3), Verdict::Match);
        assert_eq!(
            verdict(PRODUCT_PINS, m, &FIXTURE_05B.blake3),
            Verdict::Mismatch
        );
        assert!(
            TEST_PINS.contains(&QWEN25_05B),
            "test runs keep the real pin"
        );
        assert_eq!(verdict(TEST_PINS, m, &FIXTURE_05B.blake3), Verdict::Match);
        assert_eq!(verdict(TEST_PINS, m, &QWEN25_05B.blake3), Verdict::Match);
    }

    #[test]
    fn one_flipped_bit_is_a_mismatch_and_unknown_modules_are_unpinned() {
        let m = QWEN25_05B.module;
        let mut flipped = FIXTURE_05B.blake3;
        flipped[17] ^= 0x01;
        assert_eq!(verdict(TEST_PINS, m, &flipped), Verdict::Mismatch);
        assert_eq!(expected(TEST_PINS, m), Some(QWEN25_05B.blake3));
        assert_eq!(
            verdict(TEST_PINS, "/models/other.neom", &FIXTURE_05B.blake3),
            Verdict::Unpinned
        );
        assert!(is_pinned(TEST_PINS, m));
        assert!(!is_pinned(PRODUCT_PINS, "/models/reclaim-probe.bin"));
        assert_eq!(expected(PRODUCT_PINS, "/models/other.neom"), None);
    }

    #[test]
    fn only_the_boot_models_namespace_names_a_module() {
        assert_eq!(BOOT_MODEL_DIR, "/boot/models/");
        assert_eq!(
            module_suffix("/boot/models/model-qwen2.5-0.5b.neom"),
            Some("/models/model-qwen2.5-0.5b.neom")
        );
        for path in [
            "/neofs/model-qwen2.5-0.5b.neom",
            "/boot/models/",
            "/boot/models/a/b.neom",
            "/boot/modelsx/a.neom",
            "/models/model-qwen2.5-0.5b.neom",
            "boot/models/a.neom",
        ] {
            assert_eq!(module_suffix(path), None, "{path}");
        }
    }

    /// B1c byte oracle: the vocab pin is the digest Janus's two independent
    /// imports produced, laid out by hand.
    #[test]
    fn real_vocab_pin_bytes_are_the_reproduced_digest() {
        let pin = QWEN25_05B_VOCAB.blake3;
        assert_eq!(&pin[..4], &[0xe8, 0x97, 0x41, 0x21]);
        assert_eq!(&pin[28..], &[0x76, 0xc0, 0x52, 0x65]);
        assert_eq!(
            &hex_digest(&pin),
            b"e89741214838ae7af6277122c1093e899138884ea3bea4f83ff5671e76c05265"
        );
        assert!(QWEN25_05B_VOCAB.module.starts_with(VOCAB_DIR));
    }

    /// The binary vocab's pin, laid out by hand from the digest `xtask
    /// vocab-bin` printed for the real sidecar (S1, 2026-10-26).
    #[test]
    fn real_vocb_pin_bytes_are_the_converted_digest() {
        let pin = QWEN25_05B_VOCB.blake3;
        assert_eq!(&pin[..4], &[0x79, 0x73, 0xd7, 0x39]);
        assert_eq!(&pin[28..], &[0xe6, 0xce, 0xe0, 0x15]);
        assert!(QWEN25_05B_VOCB.module.starts_with(VOCAB_DIR));
        assert!(QWEN25_05B_VOCB.module.ends_with(".vocb"));
    }

    #[test]
    fn product_kernels_serve_only_the_real_vocab() {
        let m = QWEN25_05B_VOCAB.module;
        assert_eq!(PRODUCT_VOCAB_PINS, &[QWEN25_05B_VOCAB, QWEN25_05B_VOCB]);
        // The binary vocab: the real digest only, on a product kernel.
        let b = QWEN25_05B_VOCB.module;
        assert_eq!(
            verdict(PRODUCT_VOCAB_PINS, b, &FIXTURE_05B_VOCB.blake3),
            Verdict::Mismatch
        );
        assert_eq!(
            verdict(PRODUCT_VOCAB_PINS, b, &QWEN25_05B_VOCB.blake3),
            Verdict::Match
        );
        assert_eq!(
            verdict(TEST_VOCAB_PINS, b, &FIXTURE_05B_VOCB.blake3),
            Verdict::Match
        );
        // A text digest never passes for the binary module, nor the reverse.
        assert_eq!(
            verdict(PRODUCT_VOCAB_PINS, b, &QWEN25_05B_VOCAB.blake3),
            Verdict::Mismatch
        );
        assert_eq!(
            verdict(PRODUCT_VOCAB_PINS, m, &QWEN25_05B_VOCB.blake3),
            Verdict::Mismatch
        );
        assert_eq!(
            verdict(PRODUCT_VOCAB_PINS, m, &FIXTURE_05B_VOCAB.blake3),
            Verdict::Mismatch
        );
        assert_eq!(
            verdict(TEST_VOCAB_PINS, m, &FIXTURE_05B_VOCAB.blake3),
            Verdict::Match
        );
        assert_eq!(
            verdict(TEST_VOCAB_PINS, m, &QWEN25_05B_VOCAB.blake3),
            Verdict::Match
        );
        assert_eq!(
            verdict(
                TEST_VOCAB_PINS,
                "/vocab/other.vocab",
                &QWEN25_05B_VOCAB.blake3
            ),
            Verdict::Unpinned
        );
        // Model and vocab pins never name each other's modules.
        assert!(!is_pinned(PRODUCT_PINS, m));
        assert!(!is_pinned(PRODUCT_VOCAB_PINS, QWEN25_05B.module));
    }

    /// B1c: the kv-persist KAT model's pin is the digest of the file in the tree.
    #[test]
    fn kat_inventory_pin_matches_the_fixture_file() {
        let bytes: &[u8] = include_bytes!("../../../kernel/src/ai/fixtures/kv_persist_kat.neom");
        assert_eq!(bytes.len(), 382_464);
        assert_eq!(blake3::hash(bytes).as_bytes(), &KAT_INVENTORY.blake3);
        assert!(!PRODUCT_PINS.contains(&KAT_INVENTORY), "test-only");
    }
}

/// B1c (F4): the admitted-zone order guarantee, proved by brute force over
/// every alignment against the real greedy decomposition the zone uses.
#[cfg(test)]
mod guaranteed_order_tests {
    use super::module_reclaim_logic::{PAGE, Span, blocks, guaranteed_order};

    fn largest_block(start_page: u64, pages: u64) -> usize {
        let span = Span::new(start_page * PAGE, (start_page + pages) * PAGE);
        blocks(span, 20).map(|(_, order)| order).max().unwrap_or(0)
    }

    #[test]
    fn the_guarantee_holds_at_every_alignment_and_is_tight() {
        for pages in 1..=600u64 {
            let k = guaranteed_order(pages, 20).unwrap();
            // Holds: every alignment within a full cycle yields a block >= k.
            for start in 0..1024u64 {
                assert!(
                    largest_block(start, pages) >= k,
                    "pages={pages} start={start} k={k}"
                );
            }
            // Tight: one page fewer than the next threshold fails somewhere.
            let next_needed = (1u64 << (k + 2)) - 1;
            if pages + 1 == next_needed {
                assert!((0..1024).any(|start| largest_block(start, pages) < k + 1));
            }
        }
        assert_eq!(guaranteed_order(0, 16), None);
    }

    #[test]
    fn the_model_span_guarantees_15_not_16_and_the_probe_9() {
        // The pinned 0.5B module: 485,489,152 bytes = 118,527 whole pages at
        // best (its tail page is partial); a reclaimed span can lose one more
        // partial edge page.
        let model_pages = 485_489_152u64 / PAGE;
        assert_eq!(model_pages, 118_527);
        assert_eq!(guaranteed_order(model_pages, 16), Some(15));
        assert_eq!(guaranteed_order(model_pages - 1, 16), Some(15));
        // order 16 would need 2^17 - 1 = 131,071 pages.
        assert_eq!(guaranteed_order(131_071, 16), Some(16));
        assert_eq!(guaranteed_order(131_070, 16), Some(15));
        // The B1a reclaim probe (6 MiB + 2,560 B): 1,535 or 1,536 whole pages.
        assert_eq!(guaranteed_order(1_535, 14), Some(9));
        assert_eq!(guaranteed_order(1_536, 14), Some(9));
        // Capped at the allocator's maximum.
        assert_eq!(guaranteed_order(u64::MAX >> 1, 14), Some(14));
    }
}

/// B1c (F5): the inference stack's layout, guard classification, canary and
/// high-water scan.
#[cfg(test)]
mod inference_stack_tests {
    use super::inference_stack_logic::*;

    const REGION: u64 = 0xffff_ffff_9000_0000;

    /// The layout from a failed boot: KASLR put the image at
    /// 0xffffffffec3f0000 (8 MiB + 16 KiB), the 305 MiB heap sat directly
    /// above it and ran to 0xfffffffffff00000. Neither heap-side slot fits;
    /// the below-image slot must, clear of the image with a 2 MiB gap.
    #[test]
    fn region_candidates_kaslr_ceiling() {
        let image_lo = 0xffff_ffff_ec3f_0000;
        let image_hi = image_lo + 0x804_000;
        let heap_lo = 0xffff_ffff_ece0_0000;
        let heap_hi = heap_lo + 305 * 1024 * 1024;
        assert_eq!(heap_hi, 0xffff_ffff_fff0_0000);
        let [above, below_heap, below_image] = region_candidates(heap_lo, heap_hi, image_lo);
        assert_eq!(above, None, "above the heap wraps the address space");
        let b = below_heap.unwrap();
        assert!(
            b < image_hi && image_lo < b + REGION_LEN,
            "below the heap overlaps the image"
        );
        let r = below_image.unwrap();
        assert_eq!(r % ALIGN, 0);
        assert!(r + REGION_LEN + GAP <= image_lo);
        assert_eq!(r, 0xffff_ffff_ec00_0000);
    }

    /// The normal layout: the slot above the heap, aligned, with the gap.
    #[test]
    fn region_candidates_normal() {
        let heap_lo = 0xffff_ffff_8100_0000;
        let heap_hi = heap_lo + 305 * 1024 * 1024;
        let [above, _, _] = region_candidates(heap_lo, heap_hi, 0xffff_ffff_8000_0000);
        let a = above.unwrap();
        assert_eq!(a % ALIGN, 0);
        assert!(a >= heap_hi + GAP && a < heap_hi + GAP + ALIGN);
    }

    #[test]
    fn layout_and_guards() {
        assert_eq!(REGION_LEN, 65_536 + 256 * 1024 + 4096);
        assert_eq!(stack_bottom(REGION), REGION + 65_536);
        assert_eq!(stack_top(REGION), REGION + 65_536 + 262_144);
        assert_eq!(stack_top(REGION) % 16, 0);
        assert_eq!(guard_hit(REGION, REGION), Some(GuardHit::Low));
        assert_eq!(guard_hit(REGION, REGION + 65_535), Some(GuardHit::Low));
        assert_eq!(guard_hit(REGION, REGION + 65_536), None);
        assert_eq!(guard_hit(REGION, stack_top(REGION) - 1), None);
        assert_eq!(guard_hit(REGION, stack_top(REGION)), Some(GuardHit::High));
        assert_eq!(
            guard_hit(REGION, stack_top(REGION) + 4095),
            Some(GuardHit::High)
        );
        assert_eq!(guard_hit(REGION, stack_top(REGION) + 4096), None);
        assert_eq!(guard_hit(REGION, REGION - 1), None);
        assert_eq!(guard_hit(0, 0), None, "no stack mapped: nothing is a guard");
    }

    #[test]
    fn canary_and_depth() {
        let mut stack = vec![0u8; 4096];
        prepare(&mut stack);
        assert!(canary_intact(&stack));
        assert_eq!(depth_used(&stack), 0);
        // A call that used the top 100 bytes.
        for b in &mut stack[4096 - 100..] {
            *b = 0;
        }
        assert_eq!(depth_used(&stack), 100);
        assert!(canary_intact(&stack));
        // A write that reached into the canary.
        stack[CANARY_LEN - 1] = 0;
        assert!(!canary_intact(&stack));
        assert_eq!(depth_used(&stack), 4096 - (CANARY_LEN - 1));
        // A value equal to the fill byte is not counted (a lower bound).
        let mut stack = vec![0u8; 4096];
        prepare(&mut stack);
        stack[4095] = FILL_BYTE;
        assert_eq!(depth_used(&stack), 0);
    }
}
