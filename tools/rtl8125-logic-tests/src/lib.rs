// SPDX-License-Identifier: MPL-2.0
//! Host tests for the RTL8125 driver logic (`kernel/src/drivers/net/rtl8125_logic.rs`).
//!
//! The byte oracles below are laid out by hand from the bit positions in the
//! raw prior-art sources (Linux `r8169_main.c` `enum rtl_desc_bit`:
//! DescOwn = 1<<31, RingEnd = 1<<30, FirstFrag = 1<<29, LastFrag = 1<<28,
//! RxRES = 1<<21; OpenBSD `if_rgereg.h` `RGE_TDCMDSTS_*`, the same TX bits),
//! fetched with curl, never through a summarising read. They are independent
//! of the encoder: a wrong encoder and a wrong decoder cannot agree with them
//! by construction.

#![allow(dead_code)]

#[path = "../../../kernel/src/drivers/net/rtl8125_logic.rs"]
mod rtl8125_logic;

#[cfg(test)]
mod tests {
    use super::rtl8125_logic::*;

    // --- byte oracles --------------------------------------------------------

    #[test]
    fn tx_descriptor_byte_oracle() {
        // A 60-byte frame, slot not at end of ring, buffer at 0x1_2345_6780.
        // opts1 = OWN | FS | LS | 60 = 0xB000_003C.
        let want: [u8; 16] = [
            0x3c, 0x00, 0x00, 0xb0, // opts1
            0x00, 0x00, 0x00, 0x00, // opts2
            0x80, 0x67, 0x45, 0x23, 0x01, 0x00, 0x00, 0x00, // addr
        ];
        assert_eq!(
            descriptor_bytes(tx_opts1(60, false), 0, 0x1_2345_6780),
            want
        );
    }

    #[test]
    fn tx_descriptor_end_of_ring_byte_oracle() {
        // 1514 bytes (0x5EA) in the last slot: OWN | EOR | FS | LS | 0x5EA.
        let want: [u8; 16] = [
            0xea, 0x05, 0x00, 0xf0, //
            0x00, 0x00, 0x00, 0x00, //
            0x00, 0xf0, 0xff, 0x7f, 0x00, 0x00, 0x00, 0x00,
        ];
        assert_eq!(descriptor_bytes(tx_opts1(1514, true), 0, 0x7fff_f000), want);
    }

    #[test]
    fn rx_descriptor_armed_byte_oracle() {
        // A 2048-byte buffer handed to the chip in the last slot:
        // opts1 = OWN | EOR | 0x800 = 0xC000_0800.
        let want: [u8; 16] = [
            0x00, 0x08, 0x00, 0xc0, //
            0x00, 0x00, 0x00, 0x00, //
            0x00, 0x10, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        assert_eq!(descriptor_bytes(rx_opts1(2048, true), 0, 0x0020_1000), want);
        // Not the last slot: OWN only.
        assert_eq!(
            rx_opts1(2048, false).to_le_bytes(),
            [0x00, 0x08, 0x00, 0x80]
        );
    }

    #[test]
    fn rx_completion_from_chip_bytes() {
        // What the chip writes back for a 64-byte wire frame (60 + FCS):
        // OWN clear, FS | LS, length 0x40 => opts1 bytes 40 00 00 30.
        let opts1 = u32::from_le_bytes([0x40, 0x00, 0x00, 0x30]);
        assert_eq!(rx_classify(opts1), RxCompletion::Frame { len: 60 });
        // Same frame with RxRES (bit 21) set: bytes 40 00 20 30.
        let err = u32::from_le_bytes([0x40, 0x00, 0x20, 0x30]);
        assert_eq!(rx_classify(err), RxCompletion::Error { opts1: err });
        // First segment only (a frame larger than the buffer): 00 08 00 20.
        let frag = u32::from_le_bytes([0x00, 0x08, 0x00, 0x20]);
        assert_eq!(rx_classify(frag), RxCompletion::Fragment { opts1: frag });
        // OWN still set: not ours to read.
        assert_eq!(rx_classify(0x8000_0800), RxCompletion::Owned);
        // A runt shorter than its own FCS is an error, not a 0-byte frame.
        assert!(matches!(
            rx_classify(0x3000_0004),
            RxCompletion::Error { .. }
        ));
    }

    #[test]
    fn descriptor_format_stays_classic() {
        // Neither the RX v3 (32-byte) bit nor the new TX format is used.
        for v in [MacVersion::A, MacVersion::B, MacVersion::D1, MacVersion::D2] {
            assert_eq!(rx_config(v) & RX_CONFIG_DESC_V3, 0);
        }
        let tx_format = MAC_INIT_COMMON
            .iter()
            .find(|m| m.reg == MAC_OCP_TX_DESC_FORMAT)
            .expect("tx format write present");
        assert_eq!(ocp_apply(0xffff, *tx_format) & TX_DESC_NEW_FORMAT_BIT, 0);
        assert_eq!(DESC_LEN, 16);
    }

    // --- chip identification ----------------------------------------------------

    #[test]
    fn xid_decode_known_versions() {
        // TxConfig values as the chip reports them: XID in bits 30..20, plus
        // the configured low bits, which must not affect the decode.
        assert_eq!(decode_xid(0x6090_0000), Ok(MacVersion::A));
        assert_eq!(decode_xid(0x6410_0000 | TX_CONFIG), Ok(MacVersion::B));
        assert_eq!(decode_xid(0x6880_0000), Ok(MacVersion::D1));
        assert_eq!(decode_xid(0x6890_0f00), Ok(MacVersion::D2));
        for v in [MacVersion::A, MacVersion::B, MacVersion::D1, MacVersion::D2] {
            assert_eq!(decode_xid((v.xid() as u32) << 20), Ok(v));
        }
    }

    #[test]
    fn xid_decode_refuses_everything_else() {
        // RTL8126A (two steppings), 8125K, 8125BP, 8125CP, an 8168H (0x541),
        // and the masked-off bit 23 set on an otherwise-8125B value.
        for xid in [0x649u32, 0x64a, 0x68a, 0x681, 0x708, 0x541] {
            assert_eq!(decode_xid(xid << 20), Err(xid as u16), "xid {xid:#x}");
        }
        // Bits outside the 0x7cf mask are ignored (bits 24/25 are the IFG).
        assert_eq!(decode_xid((0x641 << 20) | (0x30 << 20)), Ok(MacVersion::B));
        // A dead function reads all-ones.
        assert!(mmio_dead(0xffff_ffff));
        assert_eq!(decode_xid(0xffff_ffff), Err(0x7cf));
    }

    #[test]
    fn rx_and_tx_config_values() {
        // 8125A: fetch 8 | burst 7 | accept my/bcast/mcast = 0x4000_070E.
        assert_eq!(rx_config(MacVersion::A), 0x4000_070e);
        // 8125B and later add the pause-slot bit (11).
        assert_eq!(rx_config(MacVersion::B), 0x4000_0f0e);
        assert_eq!(rx_config(MacVersion::D2), 0x4000_0f0e);
        assert_eq!(TX_CONFIG, 0x0300_0700);
    }

    // --- OCP encoding, PHY, MAC ----------------------------------------------------

    #[test]
    fn ocp_encoding() {
        // Write 0x1234 to MAC-OCP 0xe098: flag | (0xe098 >> 1) << 16 | data.
        assert_eq!(ocp_write_word(0xe098, 0x1234), Some(0xf04c_1234));
        assert_eq!(ocp_read_request(0xa420), Some(0x5210_0000));
        assert_eq!(ocp_write_word(0xa421, 0), None);
        assert_eq!(mii_ocp(mii::BMCR), 0xa400);
        assert_eq!(mii_ocp(mii::GBCR), 0xa412);
        let m = OcpModify {
            reg: 0xe614,
            clear: 0x0700,
            set: 0x0200,
        };
        assert_eq!(ocp_apply(0x0f5a, m), 0x0a5a);
    }

    #[test]
    fn phy_status_decode() {
        assert_eq!(
            decode_phy_status(0x0403),
            LinkReport {
                up: true,
                full_duplex: true,
                speed_mbps: 2500
            }
        );
        assert_eq!(decode_phy_status(0x0013).speed_mbps, 1000);
        assert_eq!(decode_phy_status(0x000a).speed_mbps, 100);
        assert!(!decode_phy_status(0x000a).full_duplex);
        // Link bit clear: down regardless of stale speed/duplex bits.
        let down = decode_phy_status(0x0411);
        assert!(!down.up && !down.full_duplex && down.speed_mbps == 0);
    }

    #[test]
    fn mac_choice() {
        let good = [0x00, 0xe0, 0x4c, 0x68, 0x01, 0x02];
        let bkp = [0x04, 0x7c, 0x16, 0xaa, 0xbb, 0xcc];
        assert_eq!(choose_mac(good, bkp), (good, MacSource::Mac0));
        assert_eq!(choose_mac([0; 6], bkp), (bkp, MacSource::Backup));
        assert_eq!(choose_mac([0xff; 6], bkp), (bkp, MacSource::Backup));
        // A multicast address is not a station address.
        assert_eq!(
            choose_mac([0x01, 0, 0x5e, 0, 0, 1], bkp),
            (bkp, MacSource::Backup)
        );
        assert_eq!(choose_mac([0; 6], [0; 6]).1, MacSource::None);
        assert_eq!(mac_registers(good), (0x684c_e000, 0x0000_0201));
    }

    #[test]
    fn pci_command_and_aspm() {
        // Firmware left I/O + memory + bus master on: bind keeps memory,
        // adds INTx disable (bit 10), drops I/O and bus master.
        assert_eq!(bind_command(0x0007), 0x0402);
        assert_eq!(bind_command(0x0007) & PCI_COMMAND_BUS_MASTER, 0);
        // Other bits (SERR) survive.
        assert_eq!(bind_command(0x0104), 0x0502);
        assert_eq!(link_control_aspm_off(0x0143), 0x0040);
    }

    // --- containment decision table -------------------------------------------------

    #[test]
    fn containment_decision_table() {
        use DmaMode::*;
        // (arena, vtd_fenced, opt_in) -> (mode, bus master)
        let rows = [
            (false, false, false, Blocked(BlockReason::NoArena), false),
            (false, true, false, Blocked(BlockReason::NoArena), false),
            (false, false, true, Blocked(BlockReason::NoArena), false),
            (false, true, true, Blocked(BlockReason::NoArena), false),
            (true, true, false, Iommu, true),
            (true, true, true, Iommu, true),
            (true, false, true, ArenaOnly, true),
            (true, false, false, Blocked(BlockReason::NoIommu), false),
        ];
        for (arena, fenced, opt_in, mode, bm) in rows {
            let got = containment(arena, fenced, opt_in);
            assert_eq!(got, mode, "arena={arena} fenced={fenced} opt_in={opt_in}");
            assert_eq!(bus_master_allowed(got), bm);
        }
        assert_eq!(
            dma_label(containment(true, false, false)),
            "blocked (no iommu)"
        );
        assert_eq!(dma_label(Iommu), "iommu");
        assert_eq!(dma_label(ArenaOnly), "arena-only");
    }

    // --- ring state machines against a register model ---------------------------------

    /// A ring of descriptors plus a fake chip. `publish` records the store
    /// order so the OWN-last rule is checkable.
    struct Model {
        opts1: Vec<u32>,
        addr: Vec<u64>,
        /// (slot, field) in store order: 0 = addr/opts2, 1 = opts1.
        stores: Vec<(usize, u8)>,
    }

    impl Model {
        fn new(n: usize) -> Self {
            Self {
                opts1: vec![0; n],
                addr: vec![0; n],
                stores: Vec::new(),
            }
        }
        /// The chip finishes TX slot `i`: clears OWN, keeps the rest.
        fn tx_complete(&mut self, i: usize) {
            assert!(
                self.opts1[i] & DESC_OWN != 0,
                "chip completing a slot it does not own"
            );
            self.opts1[i] &= !DESC_OWN;
        }
        /// The chip writes a received frame into RX slot `i` if it owns it.
        fn rx_deliver(&mut self, i: usize, wire_len: u32) -> bool {
            if self.opts1[i] & DESC_OWN == 0 {
                return false; // RX_DESC_UNAVAIL: chip has no buffer here
            }
            let eor = self.opts1[i] & DESC_EOR;
            self.opts1[i] = eor | DESC_FS | DESC_LS | wire_len;
            true
        }
    }

    impl DescMem for Model {
        fn read_opts1(&self, index: usize) -> u32 {
            self.opts1[index]
        }
        fn publish(&mut self, index: usize, opts1: u32, _opts2: u32, addr: u64) {
            self.addr[index] = addr;
            self.stores.push((index, 0));
            self.opts1[index] = opts1;
            self.stores.push((index, 1));
        }
    }

    #[test]
    fn tx_ring_wraps_and_marks_end_of_ring() {
        let mut m = Model::new(4);
        let mut tx = TxRing::new(4);
        for round in 0..3u64 {
            for i in 0..4 {
                let slot = tx
                    .post(&mut m, 0x1000 * (i as u64 + 1) + round, 60)
                    .unwrap();
                assert_eq!(slot, i);
                assert_eq!(m.opts1[slot] & DESC_EOR != 0, slot == 3);
            }
            assert!(tx.is_full());
            assert_eq!(tx.post(&mut m, 0xdead, 60), None);
            for i in 0..4 {
                m.tx_complete(i);
            }
            let mut done = Vec::new();
            assert_eq!(tx.reap(&m, 16, |s| done.push(s)), 4);
            assert_eq!(done, vec![0, 1, 2, 3]);
            assert_eq!(tx.in_flight(), 0);
        }
    }

    #[test]
    fn tx_completion_out_of_order_is_held() {
        let mut m = Model::new(4);
        let mut tx = TxRing::new(4);
        for _ in 0..3 {
            tx.post(&mut m, 0x2000, 100).unwrap();
        }
        // The chip reports slot 1 done before slot 0.
        m.tx_complete(1);
        let mut done = Vec::new();
        assert_eq!(tx.reap(&m, 16, |s| done.push(s)), 0, "slot 0 still owned");
        assert!(done.is_empty());
        m.tx_complete(0);
        assert_eq!(tx.reap(&m, 16, |s| done.push(s)), 2);
        assert_eq!(done, vec![0, 1]);
        assert_eq!(tx.in_flight(), 1, "slot 2 still with the chip");
        // Budget bounds the work per pass.
        m.tx_complete(2);
        tx.post(&mut m, 0x3000, 100).unwrap();
        m.tx_complete(3);
        assert_eq!(tx.reap(&m, 1, |s| done.push(s)), 1);
        assert_eq!(tx.reap(&m, 1, |s| done.push(s)), 1);
        assert_eq!(done, vec![0, 1, 2, 3]);
    }

    #[test]
    fn publish_writes_own_last() {
        let mut m = Model::new(2);
        let mut tx = TxRing::new(2);
        tx.post(&mut m, 0x4000, 64).unwrap();
        let mut rx = RxRing::new(2, 2048);
        rx.refill(&mut m, 0x5000).unwrap();
        // Every slot's opts1 store follows its address store.
        for pair in m.stores.chunks(2) {
            assert_eq!(pair[0].1, 0);
            assert_eq!(pair[1].1, 1);
            assert_eq!(pair[0].0, pair[1].0);
        }
    }

    /// Drive an RX ring the way the kernel does: per-slot buffers, a free
    /// pool, frames out to a "stack" that recycles them later.
    struct RxHarness {
        m: Model,
        rx: RxRing,
        slot_buf: Vec<Option<u64>>,
        pool: Vec<u64>,
        delivered: Vec<(u64, usize)>,
        dropped: usize,
        bad_length: usize,
    }

    impl RxHarness {
        fn new(n: usize, buffers: usize) -> Self {
            let mut h = Self {
                m: Model::new(n),
                rx: RxRing::new(n, 2048),
                slot_buf: vec![None; n],
                pool: (0..buffers as u64)
                    .map(|i| 0x10_0000 + i * 0x1000)
                    .rev()
                    .collect(),
                delivered: Vec::new(),
                dropped: 0,
                bad_length: 0,
            };
            h.refill_all();
            h
        }
        fn refill_all(&mut self) {
            while let Some(slot) = self.rx.next_fill_slot() {
                let Some(buf) = self.pool.pop() else { break };
                assert!(
                    self.slot_buf[slot].is_none(),
                    "refilling a slot that holds a buffer"
                );
                assert_eq!(self.rx.refill(&mut self.m, buf), Some(slot));
                self.slot_buf[slot] = Some(buf);
            }
        }
        fn reap(&mut self, budget: usize) -> usize {
            let mut events = Vec::new();
            let mut acquires = 0;
            let n = self
                .rx
                .reap(&self.m, budget, || acquires += 1, |e| events.push(e));
            assert_eq!(acquires, n, "one acquire per returned descriptor");
            for e in events {
                match e {
                    RxEvent::Frame { slot, len } => {
                        let buf = self.slot_buf[slot]
                            .take()
                            .expect("frame from an unarmed slot");
                        self.delivered.push((buf, len));
                    }
                    RxEvent::Dropped { slot, .. } => {
                        let buf = self.slot_buf[slot]
                            .take()
                            .expect("drop from an unarmed slot");
                        self.pool.push(buf);
                        self.dropped += 1;
                    }
                    RxEvent::BadLength { slot, .. } => {
                        let buf = self.slot_buf[slot]
                            .take()
                            .expect("bad length from an unarmed slot");
                        self.pool.push(buf);
                        self.bad_length += 1;
                    }
                }
            }
            self.refill_all();
            n
        }
        fn recycle_all(&mut self) {
            for (buf, _) in self.delivered.drain(..) {
                self.pool.push(buf);
            }
            self.refill_all();
        }
        fn armed(&self) -> usize {
            self.slot_buf.iter().filter(|b| b.is_some()).count()
        }
    }

    #[test]
    fn rx_ring_wraparound() {
        let mut h = RxHarness::new(4, 8);
        let mut chip = 0usize;
        for _ in 0..10 {
            for _ in 0..3 {
                assert!(h.m.rx_deliver(chip % 4, 64));
                chip += 1;
            }
            assert_eq!(h.reap(16), 3);
            h.recycle_all();
            assert_eq!(h.armed(), 4);
        }
        assert_eq!(h.dropped, 0);
    }

    #[test]
    fn rx_overflow_recovers_on_recycle() {
        // 4 slots, exactly 4 buffers: the stack holding every frame means the
        // chip runs out of descriptors (RX_DESC_UNAVAIL) and nothing wedges.
        let mut h = RxHarness::new(4, 4);
        for i in 0..4 {
            assert!(h.m.rx_deliver(i, 100));
        }
        assert_eq!(h.reap(16), 4);
        assert_eq!(h.armed(), 0, "every buffer is out with the stack");
        assert!(!h.m.rx_deliver(0, 100), "chip has no descriptor: overflow");
        // A reap with nothing armed examines nothing (never reads a slot
        // whose buffer is out).
        assert_eq!(h.reap(16), 0);
        h.recycle_all();
        assert_eq!(h.armed(), 4);
        assert!(h.m.rx_deliver(0, 100), "reception resumes after refill");
        assert_eq!(h.reap(16), 1);
        assert_eq!(h.delivered.len(), 1);
        assert_eq!(h.delivered[0].1, 96);
    }

    #[test]
    fn rx_partial_recycle_keeps_order() {
        // Recycling one buffer at a time arms slots in ring order, so the
        // chip's next slot is always the one the reap expects.
        let mut h = RxHarness::new(4, 4);
        for i in 0..4 {
            h.m.rx_deliver(i, 70);
        }
        h.reap(16);
        let held: Vec<_> = h.delivered.drain(..).collect();
        for (k, (buf, _)) in held.into_iter().enumerate() {
            h.pool.push(buf);
            h.refill_all();
            assert_eq!(h.armed(), k + 1);
        }
        for chip in 0..9 {
            assert!(h.m.rx_deliver(chip % 4, 70));
            assert_eq!(h.reap(16), 1);
            h.recycle_all();
        }
    }

    #[test]
    fn rx_error_descriptor_is_dropped_and_rearmed() {
        let mut h = RxHarness::new(4, 4);
        h.m.rx_deliver(0, 64);
        // Slot 1 comes back with the error summary set.
        h.m.opts1[1] = DESC_FS | DESC_LS | RX_ERR_SUMMARY | 64;
        h.m.rx_deliver(2, 64);
        assert_eq!(h.reap(16), 3);
        assert_eq!(h.dropped, 1);
        assert_eq!(h.delivered.len(), 2);
        h.recycle_all();
        assert_eq!(h.armed(), 4);
        // Every slot holds a distinct buffer.
        let mut bufs: Vec<_> = h.slot_buf.iter().flatten().copied().collect();
        bufs.sort();
        bufs.dedup();
        assert_eq!(bufs.len(), 4);
    }

    #[test]
    fn rx_budget_bounds_a_pass() {
        let mut h = RxHarness::new(8, 16);
        for i in 0..8 {
            h.m.rx_deliver(i, 64);
        }
        assert_eq!(h.reap(3), 3);
        assert_eq!(h.reap(3), 3);
        assert_eq!(h.reap(3), 2);
    }

    // --- review findings F1-F3 (Janus, relay #34) --------------------------------------

    #[test]
    fn rx_length_beyond_armed_buffer_is_rejected() {
        // F3: lengths are device-written metadata. The two review inputs:
        // wire 2053 (frame 2049) and the field's maximum 0x3fff, each with
        // OWN clear, FS|LS, no error, against a 2048-byte armed buffer.
        for opts1 in [0x3000_0805u32, 0x3000_3fff, 0x7000_3fff] {
            assert_eq!(
                rx_classify_bounded(opts1, 2048),
                RxCompletion::BadLength { opts1 },
                "opts1 {opts1:#x}"
            );
        }
        // Boundaries: a wire length equal to the capacity is a frame; one
        // byte over is not. A frame shorter than an Ethernet header is not.
        assert_eq!(
            rx_classify_bounded(0x3000_0800, 2048),
            RxCompletion::Frame { len: 2044 }
        );
        assert!(matches!(
            rx_classify_bounded(0x3000_0801, 2048),
            RxCompletion::BadLength { .. }
        ));
        assert!(matches!(
            rx_classify_bounded(0x3000_0011, 2048),
            RxCompletion::BadLength { .. }
        ));
        assert_eq!(
            rx_classify_bounded(0x3000_0012, 2048),
            RxCompletion::Frame { len: 14 }
        );
    }

    #[test]
    fn rx_ring_never_hands_out_an_oversized_frame() {
        // The register model completes slot 0 with 2053 bytes, slot 1 with
        // 0x3fff and slot 2 with a valid 64-byte frame. Only slot 2 reaches
        // the stack; the other buffers return to the pool and re-arm.
        let mut h = RxHarness::new(4, 4);
        h.m.opts1[0] = DESC_FS | DESC_LS | 2053;
        h.m.opts1[1] = DESC_FS | DESC_LS | 0x3fff;
        h.m.rx_deliver(2, 64);
        assert_eq!(h.reap(16), 3);
        assert_eq!(h.bad_length, 2);
        assert_eq!(h.delivered.len(), 1);
        assert!(h.delivered.iter().all(|&(_, len)| len <= 2048));
        h.recycle_all();
        assert_eq!(h.armed(), 4, "rejected buffers re-armed, none lost");
    }

    #[test]
    fn refill_accounts_for_a_fence_refusal() {
        // F1: a pool buffer the fence refuses is handed to `reject`, never
        // posted and never dropped; the next buffer fills the slot.
        let mut m = Model::new(4);
        let mut rx = RxRing::new(4, 2048);
        let bad = 0x20_0000u64; // outside the "arena" below
        let mut pool = vec![0x10_3000u64, 0x10_2000, bad, 0x10_1000, 0x10_0000];
        let in_arena = |b: &u64| (0x10_0000..0x14_0000).contains(b).then_some(*b);
        let mut rejected = Vec::new();
        let mut armed = Vec::new();
        let n = rx.refill_from(
            &mut m,
            || pool.pop(),
            in_arena,
            |b| rejected.push(b),
            |slot, b| armed.push((slot, b)),
        );
        assert_eq!(n, 4);
        assert_eq!(rejected, vec![bad]);
        assert_eq!(
            armed,
            vec![
                (0, 0x10_0000),
                (1, 0x10_1000),
                (2, 0x10_2000),
                (3, 0x10_3000)
            ]
        );
        assert!(pool.is_empty());
        assert!(
            m.addr.iter().all(|a| *a != bad),
            "refused buffer never posted"
        );
        // Conservation: every buffer is armed, rejected or still pooled.
        assert_eq!(armed.len() + rejected.len() + pool.len(), 5);
    }

    #[test]
    fn refill_with_only_refused_buffers_terminates() {
        let mut m = Model::new(4);
        let mut rx = RxRing::new(4, 2048);
        let mut pool = vec![0x20_0000u64; 6];
        let mut rejected = 0;
        let n = rx.refill_from(
            &mut m,
            || pool.pop(),
            |_| None,
            |_| rejected += 1,
            |_, _| {},
        );
        assert_eq!((n, rejected, rx.empty_slots()), (0, 6, 4));
    }

    #[test]
    fn pump_is_bounded_under_endless_rx() {
        // F2: the chip completes a new frame every time one is taken, so the
        // ring is never empty. The pump must still return after its budget.
        let mut h = RxHarness::new(16, 24);
        let mut chip = 0usize;
        for _ in 0..16 {
            assert!(h.m.rx_deliver(chip % 16, 64));
            chip += 1;
        }
        let mut taken = 0usize;
        let handled = drain_bounded(PUMP_FRAME_BUDGET, || {
            if h.delivered.is_empty() {
                h.reap(1);
            }
            let Some((buf, _)) = h.delivered.pop() else {
                return false;
            };
            taken += 1;
            h.pool.push(buf);
            h.refill_all();
            // Replenish: the chip fills the next slot before the pump looks.
            h.m.rx_deliver(chip % 16, 64);
            chip += 1;
            true
        });
        assert_eq!(handled, PUMP_FRAME_BUDGET);
        assert_eq!(taken, PUMP_FRAME_BUDGET);
        // And it stops early when there is nothing to do.
        assert_eq!(drain_bounded(PUMP_FRAME_BUDGET, || false), 0);
    }

    // --- storm detector -----------------------------------------------------------------

    #[test]
    fn storm_declared_after_fruitless_passes() {
        let mut d = StormDetector::new();
        let bits = isr::RX_FIFO_OVERFLOW;
        for _ in 0..STORM_PASSES - 1 {
            assert_eq!(d.observe(bits, false), StormVerdict::Quiet);
        }
        assert_eq!(
            d.observe(bits, false),
            StormVerdict::Storm { bits, total: 1 }
        );
    }

    #[test]
    fn progress_or_change_resets_the_streak() {
        let mut d = StormDetector::new();
        for _ in 0..STORM_PASSES * 3 {
            d.observe(isr::RX_DESC_UNAVAIL, false);
            d.observe(isr::RX_DESC_UNAVAIL, true); // descriptors moved
        }
        assert_eq!(d.storms(), 0);
        for i in 0..STORM_PASSES * 3 {
            let b = if i % 2 == 0 { isr::RX_ERR } else { isr::TX_ERR };
            d.observe(b, false);
        }
        assert_eq!(d.storms(), 0);
    }

    #[test]
    fn storm_limit_masks_the_source() {
        let mut d = StormDetector::new();
        let bits = isr::RX_FIFO_OVERFLOW;
        let mut storms = 0;
        for _ in 0..(STORM_PASSES * (STORM_LIMIT + 2)) {
            if let StormVerdict::Storm { .. } = d.observe(bits, false) {
                storms += 1;
            }
        }
        assert_eq!(storms, STORM_LIMIT);
        assert_eq!(d.masked() & bits, bits);
        // A different source is still watched.
        for _ in 0..STORM_PASSES {
            d.observe(isr::RX_ERR, false);
        }
        assert_eq!(d.storms(), STORM_LIMIT + 1);
    }

    // --- MAC init table sanity -----------------------------------------------------------

    #[test]
    fn mac_init_writes_are_even_ocp_registers() {
        for m in MAC_INIT_COMMON.iter().chain(PHY_EEE_OFF.iter()) {
            assert!(
                ocp_write_word(m.reg, 0).is_some(),
                "odd OCP reg {:#x}",
                m.reg
            );
            assert_eq!(m.set & !m.clear, 0, "set bits outside the cleared field");
        }
        for v in [MacVersion::A, MacVersion::B, MacVersion::D1, MacVersion::D2] {
            for m in mac_init_versioned(v) {
                assert_eq!(m.set & !m.clear, 0);
            }
        }
        assert_eq!(mac_init_versioned(MacVersion::B)[0].set, 0x0200);
        assert_eq!(mac_init_versioned(MacVersion::D1)[0].set, 0x0300);
    }
}
