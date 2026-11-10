// SPDX-License-Identifier: MPL-2.0
//! F11 S4: the host byte oracle (I-12) for `SYS_NET_IF_STATUS`.
//!
//! The kernel's record and rules (`kernel/src/net/if_status_logic.rs`) and
//! libneo's decoder (`userland/libneo/src/net_if_status.rs`) are included by
//! path and held to records laid out by hand here, byte by byte, independent
//! of both. A round trip between the two would pass even if both were wrong.

#[cfg(test)]
#[path = "../../../kernel/src/net/if_status_logic.rs"]
mod kernel;

#[cfg(test)]
#[path = "../../../userland/libneo/src/net_if_status.rs"]
mod libneo;

#[cfg(test)]
mod tests {
    use super::kernel::{self, Iface, Lease, NetIfStatusV1};
    use super::libneo;
    use core::mem::{offset_of, size_of};

    /// The gate's lease (`setup_menu_net`: slirp behind QEMU's `usb-net`),
    /// laid out by hand: interface 0, driver `cdc-ecm`, MAC
    /// 52:54:00:12:34:56, 10.0.2.15/24, router 10.0.2.2, DNS 10.0.2.3,
    /// 86 000 s left (expiry 86 400, clock 400). Flags: link up, lease bound,
    /// carrier, present (0b1111).
    const LEASED: [u8; 56] = [
        0x01, 0x00, 0x00, 0x00, // 0: version 1
        0x0f, 0x00, 0x00, 0x00, // 4: flags
        0x00, 0x00, 0x00, 0x00, // 8: ifindex 0
        b'c', b'd', b'c', b'-', b'e', b'c', b'm', 0x00, 0x00, 0x00, 0x00, 0x00, // 12: driver
        0x52, 0x54, 0x00, 0x12, 0x34, 0x56, // 24: mac
        0x0a, 0x00, 0x02, 0x0f, // 30: ipv4 10.0.2.15
        0x18, // 34: prefix 24
        0x00, // 35: pad
        0x0a, 0x00, 0x02, 0x02, // 36: router 10.0.2.2
        0x0a, 0x00, 0x02, 0x03, // 40: dns 10.0.2.3
        0x00, 0x00, 0x00, 0x00, // 44: pad
        0xf0, 0x4f, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, // 48: 86000 = 0x14ff0
    ];

    /// No network device: version 1, no flags, no lease (all ones).
    const NONE: [u8; 56] = [
        0x01, 0x00, 0x00, 0x00, // version
        0x00, 0x00, 0x00, 0x00, // flags
        0x00, 0x00, 0x00, 0x00, // ifindex
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // driver
        0, 0, 0, 0, 0, 0, // mac
        0, 0, 0, 0, // ipv4
        0, 0, // prefix, pad
        0, 0, 0, 0, // router
        0, 0, 0, 0, // dns
        0, 0, 0, 0, // pad
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, // no lease
    ];

    /// An RTL8125 with link but no lease, its address from the IPv4 table:
    /// interface 2, 192.0.2.7, flags link up + carrier + present (0b1101).
    const NO_LEASE: [u8; 56] = [
        0x01, 0x00, 0x00, 0x00, // version
        0x0d, 0x00, 0x00, 0x00, // flags
        0x02, 0x00, 0x00, 0x00, // ifindex 2
        b'r', b't', b'l', b'8', b'1', b'2', b'5', 0, 0, 0, 0, 0, // driver
        0x02, 0x00, 0x00, 0xaa, 0xbb, 0xcc, // mac
        0xc0, 0x00, 0x02, 0x07, // ipv4 192.0.2.7
        0, 0, // prefix 0 (no lease), pad
        0, 0, 0, 0, // router
        0, 0, 0, 0, // dns
        0, 0, 0, 0, // pad
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, // no lease
    ];

    fn bytes(v: &NetIfStatusV1) -> [u8; 56] {
        let mut out = [0u8; 56];
        // SAFETY: `NetIfStatusV1` is `repr(C)`, 56 bytes, with explicit padding
        // fields, so every byte is initialised.
        unsafe {
            core::ptr::copy_nonoverlapping(
                (v as *const NetIfStatusV1).cast::<u8>(),
                out.as_mut_ptr(),
                56,
            )
        };
        out
    }

    const ECM: Iface<'static> = Iface {
        ifindex: 0,
        driver: "cdc-ecm",
        mac: [0x52, 0x54, 0x00, 0x12, 0x34, 0x56],
        link_up: true,
        carrier: true,
    };

    const GATE_LEASE: Lease = Lease {
        ifindex: 0,
        ip: [10, 0, 2, 15],
        mask: [255, 255, 255, 0],
        router: Some([10, 0, 2, 2]),
        dns: Some([10, 0, 2, 3]),
        expiry: 86_400,
        now: 400,
    };

    #[test]
    fn the_layout_field_by_field() {
        assert_eq!(size_of::<NetIfStatusV1>(), 56);
        assert_eq!(kernel::NET_IF_STATUS_V1_SIZE, 56);
        assert_eq!(libneo::RECORD_LEN, 56);
        assert_eq!(offset_of!(NetIfStatusV1, version), 0);
        assert_eq!(offset_of!(NetIfStatusV1, flags), 4);
        assert_eq!(offset_of!(NetIfStatusV1, ifindex), 8);
        assert_eq!(offset_of!(NetIfStatusV1, driver), 12);
        assert_eq!(offset_of!(NetIfStatusV1, mac), 24);
        assert_eq!(offset_of!(NetIfStatusV1, ipv4), 30);
        assert_eq!(offset_of!(NetIfStatusV1, prefix), 34);
        assert_eq!(offset_of!(NetIfStatusV1, _pad0), 35);
        assert_eq!(offset_of!(NetIfStatusV1, router), 36);
        assert_eq!(offset_of!(NetIfStatusV1, dns), 40);
        assert_eq!(offset_of!(NetIfStatusV1, _pad1), 44);
        assert_eq!(offset_of!(NetIfStatusV1, lease_secs_left), 48);
        // The flag bits agree on both sides.
        assert_eq!(
            [
                kernel::FLAG_LINK_UP,
                kernel::FLAG_LEASE_BOUND,
                kernel::FLAG_CARRIER,
                kernel::FLAG_PRESENT,
                kernel::FLAG_STALE
            ],
            [1, 2, 4, 8, 16]
        );
        assert_eq!(
            [
                libneo::FLAG_LINK_UP,
                libneo::FLAG_LEASE_BOUND,
                libneo::FLAG_CARRIER,
                libneo::FLAG_PRESENT,
                libneo::FLAG_STALE
            ],
            [1, 2, 4, 8, 16]
        );
    }

    #[test]
    fn the_kernel_writes_the_hand_laid_bytes() {
        let leased = kernel::build(Some(ECM), Some(GATE_LEASE), Some([9, 9, 9, 9]));
        assert_eq!(bytes(&leased), LEASED);
        assert_eq!(bytes(&kernel::build(None, None, None)), NONE);
        assert_eq!(bytes(&NetIfStatusV1::none()), NONE);
        let rtl = Iface {
            ifindex: 2,
            driver: "rtl8125",
            mac: [0x02, 0x00, 0x00, 0xaa, 0xbb, 0xcc],
            link_up: true,
            carrier: true,
        };
        assert_eq!(
            bytes(&kernel::build(Some(rtl), None, Some([192, 0, 2, 7]))),
            NO_LEASE
        );
    }

    #[test]
    fn libneo_reads_the_hand_laid_bytes() {
        let s = libneo::decode(&LEASED).expect("v1");
        assert!(s.present() && s.link_up() && s.lease_bound());
        assert_eq!(s.flags, 0x0f);
        assert_eq!(s.ifindex, 0);
        assert_eq!(s.driver(), "cdc-ecm");
        assert_eq!(s.mac, [0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);
        assert_eq!(s.address(), Some([10, 0, 2, 15]));
        assert_eq!(s.prefix, 24);
        assert_eq!(s.router, [10, 0, 2, 2]);
        assert_eq!(s.dns, [10, 0, 2, 3]);
        assert_eq!(s.lease_secs_left(), Some(86_000));

        let none = libneo::decode(&NONE).expect("v1");
        assert!(!none.present() && !none.link_up() && !none.lease_bound());
        assert_eq!(none.driver(), "");
        assert_eq!(none.address(), None);
        assert_eq!(none.lease_secs_left(), None);

        let rtl = libneo::decode(&NO_LEASE).expect("v1");
        assert!(rtl.present() && rtl.link_up() && !rtl.lease_bound());
        assert_eq!(rtl.driver(), "rtl8125");
        assert_eq!(rtl.address(), Some([192, 0, 2, 7]));
        assert_eq!(rtl.lease_secs_left(), None);
    }

    /// The dispatch's KAT: a snapshot with no lease yields `lease=none` (no
    /// bound flag, all-ones time), whatever the address.
    #[test]
    fn no_lease_is_lease_none() {
        let s = kernel::build(Some(ECM), None, None);
        assert_eq!(s.flags & kernel::FLAG_LEASE_BOUND, 0);
        assert_eq!(s.lease_secs_left, kernel::LEASE_NONE);
        assert_eq!(s.ipv4, [0; 4]);
        assert_eq!(s.prefix, 0);
        assert_eq!(libneo::decode(&bytes(&s)).unwrap().lease_secs_left(), None);
        // A lease on another interface is not this one's.
        let other = Lease {
            ifindex: 1,
            ..GATE_LEASE
        };
        let s = kernel::build(Some(ECM), Some(other), Some([10, 0, 2, 99]));
        assert_eq!(s.flags & kernel::FLAG_LEASE_BOUND, 0);
        assert_eq!(s.ipv4, [10, 0, 2, 99]);
        assert_eq!(s.lease_secs_left, kernel::LEASE_NONE);
        // An expired lease reads 0 left, not a wrapped count.
        let late = Lease {
            now: 90_000,
            ..GATE_LEASE
        };
        assert_eq!(
            kernel::build(Some(ECM), Some(late), None).lease_secs_left,
            0
        );
    }

    /// `EINVAL` on a short buffer: the kernel refuses any length below 56.
    #[test]
    fn a_short_buffer_is_refused() {
        assert!(!kernel::len_ok(0));
        assert!(!kernel::len_ok(55));
        assert!(kernel::len_ok(56));
        assert!(kernel::len_ok(64));
    }

    #[test]
    fn link_down_and_carrier_are_their_own_bits() {
        let down = Iface {
            link_up: false,
            carrier: false,
            ..ECM
        };
        let s = kernel::build(Some(down), None, None);
        assert_eq!(s.flags, kernel::FLAG_PRESENT);
        let d = libneo::decode(&bytes(&s)).unwrap();
        assert!(d.present() && !d.link_up());
    }

    #[test]
    fn the_default_interface() {
        assert_eq!(kernel::choose(&[], None), None);
        assert_eq!(kernel::choose(&[], Some(0)), None);
        // The leased one, if registered.
        assert_eq!(kernel::choose(&[(0, true), (1, true)], Some(1)), Some(1));
        // A lease on an unregistered index falls back to the first up.
        assert_eq!(kernel::choose(&[(0, false), (1, true)], Some(7)), Some(1));
        // No lease: the first up, else the first.
        assert_eq!(kernel::choose(&[(0, false), (1, true)], None), Some(1));
        assert_eq!(kernel::choose(&[(3, false), (4, false)], None), Some(3));
    }

    #[test]
    fn prefix_lengths() {
        assert_eq!(kernel::prefix_len([255, 255, 255, 0]), 24);
        assert_eq!(kernel::prefix_len([255, 255, 240, 0]), 20);
        assert_eq!(kernel::prefix_len([0, 0, 0, 0]), 0);
        assert_eq!(kernel::prefix_len([255, 255, 255, 255]), 32);
        // Non-contiguous: up to the first zero.
        assert_eq!(kernel::prefix_len([255, 0, 255, 0]), 8);
    }

    #[test]
    fn a_long_driver_name_is_cut_at_twelve() {
        let long = Iface {
            driver: "a-very-long-driver",
            ..ECM
        };
        let s = kernel::build(Some(long), None, None);
        assert_eq!(&s.driver, b"a-very-long-");
        assert_eq!(libneo::decode(&bytes(&s)).unwrap().driver(), "a-very-long-");
    }

    /// Janus, #100 relay 382 (F1): the sources the record reads, each with
    /// its writer, and the network-status generation every writer bumps
    /// after its write (as the kernel's writers do inside their locks). A
    /// writer can run between any two of the reader's reads.
    mod world {
        use super::*;
        use std::cell::{Cell, RefCell};
        use std::collections::VecDeque;

        pub struct World {
            pub generation: Cell<u64>,
            pub leased: Cell<bool>,
            pub link: Cell<bool>,
            pub carrier: Cell<bool>,
            /// Writers, run one per gap between reads (`step`).
            pub writers: RefCell<VecDeque<fn(&World)>>,
            /// Writers run again whenever the queue runs dry (a network that
            /// keeps changing).
            pub forever: Option<fn(&World)>,
            pub reads: Cell<usize>,
            /// Every state the sources held, `(lease, link, carrier)`, in order.
            pub held: RefCell<Vec<(bool, bool, bool)>>,
        }

        impl World {
            /// Janus's start: interface 0 up, carrier up, the gate's lease.
            pub fn up() -> World {
                World {
                    generation: Cell::new(0),
                    leased: Cell::new(true),
                    link: Cell::new(true),
                    carrier: Cell::new(true),
                    writers: RefCell::new(VecDeque::new()),
                    forever: None,
                    reads: Cell::new(0),
                    held: RefCell::new(vec![(true, true, true)]),
                }
            }

            /// After a write: the generation moves, and the new state is one
            /// the sources held.
            fn bump(&self) {
                self.generation.set(self.generation.get() + 1);
                self.held.borrow_mut().push((
                    self.leased.get(),
                    self.link.get(),
                    self.carrier.get(),
                ));
            }

            /// The record is a state the sources really held.
            pub fn coherent(&self, r: &NetIfStatusV1) -> bool {
                let seen = (
                    r.flags & kernel::FLAG_LEASE_BOUND != 0,
                    r.flags & kernel::FLAG_LINK_UP != 0,
                    r.flags & kernel::FLAG_CARRIER != 0,
                );
                self.held.borrow().contains(&seen)
            }

            /// The gap between two reads: the next writer, if any, runs.
            fn step(&self) {
                let next = self.writers.borrow_mut().pop_front();
                if let Some(w) = next.or(self.forever) {
                    w(self);
                }
            }

            /// One read, in the kernel's order: the carrier first, then the
            /// lease, then the registry's link, a writer in each gap.
            pub fn read(&self) -> NetIfStatusV1 {
                self.reads.set(self.reads.get() + 1);
                let carrier = self.carrier.get();
                self.step();
                let lease = self.leased.get().then_some(GATE_LEASE);
                self.step();
                let link_up = self.link.get();
                self.step();
                kernel::build(
                    Some(Iface {
                        link_up,
                        carrier,
                        ..ECM
                    }),
                    lease,
                    None,
                )
            }
        }

        /// The lease is invalidated (an ACD conflict or expiry).
        pub fn drop_lease(w: &World) {
            w.leased.set(false);
            w.bump();
        }

        /// The link goes down: the driver's carrier, then the registry's
        /// refresh.
        pub fn link_down(w: &World) {
            w.carrier.set(false);
            w.link.set(false);
            w.bump();
        }

        /// Janus's gap: the lease drops, then the link goes down, between the
        /// reader's lease read and its link read.
        pub fn drop_then_down(w: &World) {
            drop_lease(w);
            link_down(w);
        }

        /// A no-op gap.
        pub fn nothing(_: &World) {}

        /// The link flaps: down, and back up with its lease.
        pub fn flap(w: &World) {
            let up = !w.link.get();
            w.carrier.set(up);
            w.link.set(up);
            w.leased.set(up);
            w.bump();
        }
    }

    /// Without the generation, Janus's schedule returns a record no state
    /// held: the lease bound, the link down (flags 0x0a, `PRESENT |
    /// LEASE_BOUND`).
    #[test]
    fn without_the_generation_a_mixed_record_appears() {
        let w = world::World::up();
        // Carrier read (up); then the lease is read (bound); then the lease
        // drops and the link goes down before the link is read. The states
        // held: (lease, link) = (1,1) -> (0,1) -> (0,0).
        w.writers
            .borrow_mut()
            .extend([world::nothing as fn(&world::World), world::drop_then_down]);
        let mixed = w.read();
        // Carrier as first read (up), the lease, the link down.
        assert_eq!(
            mixed.flags,
            kernel::FLAG_PRESENT | kernel::FLAG_LEASE_BOUND | kernel::FLAG_CARRIER
        );
        assert_eq!(mixed.ipv4, [10, 0, 2, 15]);
        assert_eq!(mixed.lease_secs_left, 86_000);
        assert!(!w.coherent(&mixed));
    }

    /// With it, the same schedule is seen and read again: the record is the
    /// state after both writes (no lease, link down), not a mix.
    #[test]
    fn with_the_generation_the_record_is_one_state() {
        let w = world::World::up();
        w.writers
            .borrow_mut()
            .extend([world::nothing as fn(&world::World), world::drop_then_down]);
        let r = kernel::read_consistent(|| w.generation.get(), || Some(w.read()));
        assert_eq!(w.reads.get(), 2, "the mixed read is read again");
        assert_eq!(r.flags, kernel::FLAG_PRESENT);
        assert_eq!(r.lease_secs_left, kernel::LEASE_NONE);
        assert!(w.coherent(&r));
        assert_eq!(r.flags & kernel::FLAG_STALE, 0);
    }

    /// A writer in every gap of every read, in every order: each record
    /// returned is a state the sources held, or it says it is stale.
    #[test]
    fn every_interleaving_is_one_state_or_flagged() {
        let writers: [fn(&world::World); 5] = [
            world::nothing,
            world::drop_lease,
            world::link_down,
            world::drop_then_down,
            world::flap,
        ];
        let mut mixed_without = 0;
        for a in writers {
            for b in writers {
                for c in writers {
                    // Without the generation: one read.
                    let w = world::World::up();
                    w.writers.borrow_mut().extend([a, b, c]);
                    if !w.coherent(&w.read()) {
                        mixed_without += 1;
                    }
                    // With it.
                    let w = world::World::up();
                    w.writers.borrow_mut().extend([a, b, c]);
                    let r = kernel::read_consistent(|| w.generation.get(), || Some(w.read()));
                    assert!(
                        w.coherent(&r) || r.flags & kernel::FLAG_STALE != 0,
                        "flags={:#x}",
                        r.flags
                    );
                }
            }
        }
        // The schedules do produce mixed records without it.
        assert!(mixed_without > 0);
    }

    /// A network that never settles: the reader stops after its bound and
    /// says so; it never spins.
    #[test]
    fn a_network_that_keeps_changing_is_flagged_stale_after_the_bound() {
        let mut w = world::World::up();
        w.forever = Some(world::flap);
        let r = kernel::read_consistent(|| w.generation.get(), || Some(w.read()));
        assert_eq!(w.reads.get(), kernel::READ_ATTEMPTS);
        assert_ne!(r.flags & kernel::FLAG_STALE, 0);
        let d = libneo::decode(&bytes(&r)).unwrap();
        assert!(d.stale());
        // A read that never yields a record is flagged, and empty.
        let none = kernel::read_consistent(|| 0, || None);
        assert_eq!(none.flags, kernel::FLAG_STALE);
    }

    /// A quiet network is read once.
    #[test]
    fn a_quiet_network_is_read_once() {
        let w = world::World::up();
        let r = kernel::read_consistent(|| w.generation.get(), || Some(w.read()));
        assert_eq!(w.reads.get(), 1);
        assert_eq!(bytes(&r), LEASED);
    }

    #[test]
    fn libneo_refuses_another_version_or_dirty_padding() {
        let mut v2 = LEASED;
        v2[0] = 2;
        assert_eq!(libneo::decode(&v2), None);
        let mut pad = LEASED;
        pad[35] = 1;
        assert_eq!(libneo::decode(&pad), None);
        let mut pad = LEASED;
        pad[46] = 1;
        assert_eq!(libneo::decode(&pad), None);
    }
}
