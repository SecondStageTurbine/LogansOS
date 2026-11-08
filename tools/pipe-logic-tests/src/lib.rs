// SPDX-License-Identifier: MPL-2.0
//! Host KATs for `kernel/src/ipc/pipe_logic.rs` (Lane 1, L1a.1; OT-88 §3.2).
//!
//! The real module is mounted through `#[path]`, so these tests exercise the
//! exact code the kernel will run. The expected outcomes are written out by
//! hand from OT-88 §3.2's table (EOF and EPIPE ordering, the last endpoint's
//! wake, counts across grants) and L1a's §3.3 (who blocks), not computed
//! from the code under test.

#![allow(dead_code)]

#[path = "../../../kernel/src/ipc/pipe_logic.rs"]
mod pipe_logic;

/// libneo's side of the pipe ABI, mounted the same way, so the byte oracle
/// below pins both halves independently (Dexter #56 F3).
#[path = "../../../userland/libneo/src/pipe_abi.rs"]
mod libneo_pipe_abi;

#[cfg(test)]
mod tests {
    use super::pipe_logic::*;

    const NONE: Wake = Wake {
        readers: false,
        writers: false,
    };
    const READERS: Wake = Wake {
        readers: true,
        writers: false,
    };
    const WRITERS: Wake = Wake {
        readers: false,
        writers: true,
    };
    /// The last close of an end wakes both sides (Dexter #56 F1).
    const BOTH: Wake = Wake {
        readers: true,
        writers: true,
    };

    struct P {
        pipe: Pipe,
        storage: Vec<u8>,
    }

    impl P {
        fn new(capacity: usize) -> Self {
            Self {
                pipe: Pipe::new(capacity).unwrap(),
                storage: vec![0; capacity],
            }
        }
        fn write(&mut self, bytes: &[u8]) -> (WriteOutcome, Wake) {
            self.pipe.write(&mut self.storage, bytes).unwrap()
        }
        fn read(&mut self, n: usize) -> (ReadOutcome, Wake, Vec<u8>) {
            let mut dst = vec![0; n];
            let (o, w) = self.pipe.read(&self.storage, &mut dst).unwrap();
            let got = match o {
                ReadOutcome::Read(k) => dst[..k].to_vec(),
                _ => Vec::new(),
            };
            (o, w, got)
        }
    }

    #[test]
    fn capacities() {
        assert_eq!(STDIN_PIPE_CAPACITY, 4096);
        assert_eq!(STDOUT_PIPE_CAPACITY, 65536);
        assert_eq!(Pipe::new(0), Err(PipeError::BadCapacity));
        assert_eq!(
            Pipe::new(MAX_PIPE_CAPACITY + 1),
            Err(PipeError::BadCapacity)
        );
        assert!(Pipe::new(MAX_PIPE_CAPACITY).is_ok());
        let mut p = Pipe::new(8).unwrap();
        let mut wrong = [0u8; 4];
        assert_eq!(p.write(&mut wrong, b"x"), Err(PipeError::BadCapacity));
    }

    #[test]
    fn bytes_round_trip_in_order() {
        let mut p = P::new(16);
        assert_eq!(p.write(b"hello"), (WriteOutcome::Written(5), READERS));
        assert_eq!(p.write(b" world"), (WriteOutcome::Written(6), NONE));
        // Every read that frees space wakes the writers (step 2: atomic
        // writers park on a pipe that isn't full).
        assert_eq!(p.read(3), (ReadOutcome::Read(3), WRITERS, b"hel".to_vec()));
        assert_eq!(
            p.read(100),
            (ReadOutcome::Read(8), WRITERS, b"lo world".to_vec())
        );
        assert_eq!(p.read(1).0, ReadOutcome::WouldBlock);
    }

    /// The ring wraps: bytes written across the end come back in order.
    #[test]
    fn wraparound() {
        let mut p = P::new(8);
        p.write(b"abcdef");
        p.read(5); // head at 5, 1 byte buffered
        assert_eq!(p.write(b"GHIJKLM").0, WriteOutcome::Written(7)); // fills to 8
        assert_eq!(p.read(8).2, b"fGHIJKLM".to_vec());
    }

    /// A write larger than the capacity stores what fits (partial success);
    /// a full pipe would block; draining wakes the writers.
    #[test]
    fn partial_write_then_block_then_wake() {
        let mut p = P::new(4);
        assert_eq!(p.write(b"abcdef"), (WriteOutcome::Written(4), READERS));
        assert!(!p.pipe.writable());
        assert_eq!(p.write(b"g"), (WriteOutcome::WouldBlock, NONE));
        assert_eq!(p.read(1), (ReadOutcome::Read(1), WRITERS, b"a".to_vec()));
        // One byte of room in a 4-byte pipe: a 4-byte write is atomic and
        // would wait, so the write end is not ready yet (#56 F2).
        assert!(!p.pipe.writable());
        // Not full any more, and the read still wakes the writers.
        assert_eq!(p.read(1).1, WRITERS);
        assert_eq!(p.read(2).1, WRITERS);
        assert!(p.pipe.writable());
        // An empty pipe's read frees nothing and wakes nobody.
        let mut q = P::new(4);
        q.pipe.close_endpoint(End::Write).unwrap();
        assert_eq!(q.read(1).1, NONE);
    }

    /// Step 2 §2a: a write of at most PIPE_ATOMIC bytes that fits the
    /// capacity is stored whole or not at all.
    #[test]
    fn small_writes_are_atomic() {
        assert_eq!(PIPE_ATOMIC, 512);
        let mut p = P::new(8);
        p.write(b"abcde"); // 3 bytes of room
        assert_eq!(p.write(b"wxyz"), (WriteOutcome::WouldBlock, NONE));
        assert_eq!(p.pipe.buffered(), 5);
        // One byte read: 4 bytes of room, and the writer is woken for it.
        assert_eq!(p.read(1).1, WRITERS);
        assert_eq!(p.write(b"wxyz").0, WriteOutcome::Written(4));
        assert_eq!(p.read(8).2, b"bcdewxyz".to_vec());
    }

    /// A write larger than PIPE_ATOMIC, or than the capacity, may be split.
    #[test]
    fn large_writes_may_be_split() {
        let mut p = P::new(1024);
        p.write(&[1; 1000]); // 24 bytes of room
        assert_eq!(p.write(&[2; 600]).0, WriteOutcome::Written(24));
        let mut q = P::new(4);
        q.write(b"ab");
        // 3 bytes fit the capacity of 4, so the write is atomic: it waits.
        assert_eq!(q.write(b"xyz").0, WriteOutcome::WouldBlock);
        // 6 bytes > capacity: not atomic, stores what fits.
        assert_eq!(q.write(b"uvwxyz").0, WriteOutcome::Written(2));
    }

    /// OT-88 §3.2: "last writer closed → reader gets EOF after the buffer
    /// drains".
    #[test]
    fn eof_comes_after_the_buffer_drains() {
        let mut p = P::new(16);
        p.write(b"tail");
        assert_eq!(p.pipe.close_endpoint(End::Write), Ok(BOTH));
        assert!(p.pipe.readable());
        assert_eq!(p.read(2), (ReadOutcome::Read(2), WRITERS, b"ta".to_vec()));
        assert_eq!(p.read(9), (ReadOutcome::Read(2), WRITERS, b"il".to_vec()));
        assert_eq!(p.read(9).0, ReadOutcome::Eof);
        assert_eq!(p.read(9).0, ReadOutcome::Eof);
    }

    /// OT-88 §3.2: "last reader closed → writer gets EPIPE at once", even
    /// with space, and even when it was parked on a full pipe.
    #[test]
    fn epipe_is_immediate() {
        let mut p = P::new(4);
        p.write(b"full");
        assert_eq!(p.write(b"x").0, WriteOutcome::WouldBlock);
        assert_eq!(p.pipe.close_endpoint(End::Read), Ok(BOTH));
        assert!(p.pipe.writable());
        assert_eq!(p.write(b"x").0, WriteOutcome::BrokenPipe);
        // The unreadable bytes are dropped.
        assert_eq!(p.pipe.buffered(), 0);
    }

    /// OT-88 §3.2 "wake on the last endpoint": only the last close of an
    /// end wakes the other side.
    #[test]
    fn only_the_last_endpoint_wakes() {
        let mut p = P::new(8);
        p.pipe.add_endpoint(End::Write).unwrap();
        assert_eq!(p.pipe.close_endpoint(End::Write), Ok(NONE));
        assert_eq!(p.read(1).0, ReadOutcome::WouldBlock); // a writer remains
        assert_eq!(p.pipe.close_endpoint(End::Write), Ok(BOTH));
        assert_eq!(p.read(1).0, ReadOutcome::Eof);
    }

    /// OT-88 §3.2 "counts across grants": grant the write end away and close
    /// the original; the reader still waits. Close the granted one: EOF.
    /// Both ends gone: the object is dead (its charge returns to zero).
    #[test]
    fn counts_are_per_endpoint_across_grants() {
        let mut p = P::new(8);
        p.pipe.add_endpoint(End::Write).unwrap(); // the grant
        p.pipe.close_endpoint(End::Write).unwrap(); // the original
        assert_eq!(p.pipe.writers(), 1);
        assert!(!p.pipe.readable());
        assert_eq!(p.read(1).0, ReadOutcome::WouldBlock);
        p.pipe.close_endpoint(End::Write).unwrap(); // the granted one
        assert_eq!(p.read(1).0, ReadOutcome::Eof);
        assert!(!p.pipe.is_dead());
        p.pipe.close_endpoint(End::Read).unwrap();
        assert!(p.pipe.is_dead());
    }

    /// Dexter #46 1b: an end at count 0 can't be reopened.
    #[test]
    fn a_closed_end_stays_closed() {
        let mut p = Pipe::new(8).unwrap();
        p.close_endpoint(End::Write).unwrap();
        assert_eq!(p.add_endpoint(End::Write), Err(PipeError::EndClosed));
        assert_eq!(p.writers(), 0);
        // The read end still grants.
        p.add_endpoint(End::Read).unwrap();
        assert_eq!(p.readers(), 2);
    }

    /// The kernel's shape: counts start at 0 and each handle install adds
    /// one; the ends are open until a count drops back to 0.
    #[test]
    fn an_unattached_pipe_attaches_then_closes() {
        let mut p = Pipe::new_unattached(8).unwrap();
        assert_eq!((p.readers(), p.writers()), (0, 0));
        p.add_endpoint(End::Read).unwrap();
        p.add_endpoint(End::Write).unwrap();
        assert_eq!((p.readers(), p.writers()), (1, 1));
        p.close_endpoint(End::Write).unwrap();
        assert_eq!(p.add_endpoint(End::Write), Err(PipeError::EndClosed));
    }

    /// Dexter #46 note 2: the count bound.
    #[test]
    fn endpoint_counts_cannot_overflow() {
        let mut p = Pipe::with_endpoints(8, u32::MAX, 1).unwrap();
        assert_eq!(p.add_endpoint(End::Read), Err(PipeError::TooManyEndpoints));
        assert_eq!(p.readers(), u32::MAX);
        assert!(Pipe::with_endpoints(0, 1, 1).is_err());
    }

    /// Dexter #46 note 4: the wrong storage length on the read path too.
    #[test]
    fn read_checks_the_storage_length() {
        let mut p = Pipe::new(8).unwrap();
        let mut dst = [0u8; 4];
        assert_eq!(p.read(&[0u8; 7], &mut dst), Err(PipeError::BadCapacity));
    }

    /// Dexter #46 note 4: the last reader goes, then the last writer: EPIPE
    /// first, then the object is dead with nothing to wake.
    #[test]
    fn reader_then_writer_close() {
        let mut p = P::new(4);
        p.write(b"ab");
        assert_eq!(p.pipe.close_endpoint(End::Read), Ok(BOTH));
        assert_eq!(p.write(b"c").0, WriteOutcome::BrokenPipe);
        assert_eq!(p.pipe.close_endpoint(End::Write), Ok(BOTH));
        assert!(p.pipe.is_dead());
    }

    #[test]
    fn a_double_close_is_refused() {
        let mut p = Pipe::new(8).unwrap();
        p.close_endpoint(End::Read).unwrap();
        assert_eq!(p.close_endpoint(End::Read), Err(PipeError::NoSuchEndpoint));
        assert_eq!(p.readers(), 0);
    }

    /// Readiness, as `SYS_WAIT_ANY` will report it.
    #[test]
    fn readiness() {
        let mut p = P::new(2);
        assert!(!p.pipe.readable());
        assert!(p.pipe.writable());
        p.write(b"ab");
        assert!(p.pipe.readable());
        assert!(!p.pipe.writable());
        let mut q = P::new(2);
        q.pipe.close_endpoint(End::Write).unwrap();
        assert!(q.pipe.readable(), "EOF is readable");
        let mut r = P::new(2);
        r.write(b"ab");
        r.pipe.close_endpoint(End::Read).unwrap();
        assert!(r.pipe.writable(), "EPIPE is writable");
    }

    /// Dexter #56 F1: a thread parked on an end whose last handle a sibling
    /// closed is woken by that close, and its retry is refused, on either
    /// end, instead of parking again on an end nothing can wake.
    #[test]
    fn a_closed_end_refuses_its_own_read_and_write() {
        let mut p = P::new(8);
        assert_eq!(p.read(4).0, ReadOutcome::WouldBlock); // a reader parks
        assert_eq!(p.pipe.close_endpoint(End::Read), Ok(BOTH));
        let mut dst = [0u8; 4];
        assert_eq!(p.pipe.read(&p.storage, &mut dst), Err(PipeError::EndClosed));
        let mut q = P::new(4);
        q.write(b"full");
        assert_eq!(q.write(b"x").0, WriteOutcome::WouldBlock); // a writer parks
        assert_eq!(q.pipe.close_endpoint(End::Write), Ok(BOTH));
        assert_eq!(
            q.pipe.write(&mut q.storage, b"x"),
            Err(PipeError::EndClosed)
        );
        // The other end still works: the reader drains, then EOF.
        assert_eq!(q.read(8).2, b"full".to_vec());
        assert_eq!(q.read(8).0, ReadOutcome::Eof);
    }

    /// Dexter #56 F2: a write end is ready only when a `PIPE_ATOMIC`-sized
    /// write (or, for a smaller pipe, a capacity-sized one) would fit, so a
    /// `SYS_WAIT_ANY` + non-blocking write loop can't spin. The reviewer's
    /// case: capacity 512, 400 buffered, 112 free.
    #[test]
    fn write_readiness_means_an_atomic_write_fits() {
        let mut p = P::new(512);
        p.write(&[1; 400]);
        assert!(!p.pipe.writable());
        assert_eq!(p.write(&[2; 300]).0, WriteOutcome::WouldBlock);
        p.read(400);
        assert!(p.pipe.writable());
        assert_eq!(p.write(&[2; 300]).0, WriteOutcome::Written(300));
        // A larger pipe: 512 free is enough, with bytes still buffered.
        let mut q = P::new(1024);
        q.write(&[1; 513]);
        assert!(!q.pipe.writable()); // 511 free
        q.read(1);
        assert!(q.pipe.writable()); // 512 free
        assert_eq!(q.write(&[2; 512]).0, WriteOutcome::Written(512));
    }

    /// Chad #56 item 5: `SYS_PIPE_CREATE` makes no pipe smaller than
    /// `PIPE_ATOMIC`, so a write of at most 512 bytes always fits the
    /// capacity and is always atomic.
    #[test]
    fn create_refuses_a_capacity_below_pipe_atomic() {
        assert_eq!(MIN_PIPE_CAPACITY, 512);
        assert!(!create_capacity_ok(0));
        assert!(!create_capacity_ok(1));
        assert!(!create_capacity_ok(511));
        assert!(create_capacity_ok(512));
        assert!(create_capacity_ok(STDIN_PIPE_CAPACITY));
        assert!(create_capacity_ok(STDOUT_PIPE_CAPACITY));
        assert!(create_capacity_ok(MAX_PIPE_CAPACITY));
        assert!(!create_capacity_ok(MAX_PIPE_CAPACITY + 1));
        // At the floor, a 512-byte write into an empty pipe goes in whole,
        // and the write end is ready exactly when it would.
        let mut storage = vec![0u8; 512];
        let mut p = Pipe::new(512).unwrap();
        assert!(p.writable());
        assert_eq!(
            p.write(&mut storage, &[7; 512]).unwrap().0,
            WriteOutcome::Written(512)
        );
        assert!(!p.writable());
    }

    /// The byte oracle for the pipe ABI (Dexter #56 F3), hand-laid: the
    /// read end in the low 32 bits of `SYS_PIPE_CREATE`'s return, the write
    /// end in the high 32; `PIPE_NONBLOCK` is bit 0.
    #[test]
    fn abi_byte_oracle() {
        assert_eq!(pack_ends(0x1122_3344, 0x5566_7788), 0x5566_7788_1122_3344);
        assert_eq!(
            pack_ends(0x1122_3344, 0x5566_7788).to_le_bytes(),
            [0x44, 0x33, 0x22, 0x11, 0x88, 0x77, 0x66, 0x55]
        );
        assert_eq!(
            unpack_ends(0x5566_7788_1122_3344),
            (0x1122_3344, 0x5566_7788)
        );
        assert_eq!(pack_ends(0, 1), 0x0000_0001_0000_0000);
        assert_eq!(PIPE_NONBLOCK, 1);
        // libneo's half, against the same hand-laid bytes.
        use super::libneo_pipe_abi as user;
        let packed = u64::from_le_bytes([0x44, 0x33, 0x22, 0x11, 0x88, 0x77, 0x66, 0x55]);
        assert_eq!(user::pipe_split_ends(packed), (0x1122_3344, 0x5566_7788));
        assert_eq!(user::PIPE_NONBLOCK, 1);
        assert_eq!(
            (
                user::SYS_PIPE_CREATE,
                user::SYS_PIPE_READ,
                user::SYS_PIPE_WRITE
            ),
            (0x1BA, 0x1BB, 0x1BC)
        );
    }

    /// The zero-length contract (Dexter #46 1a; step-2 THINK §2a), as
    /// Linux's: a write is EPIPE with no reader, else 0, even on a full
    /// pipe; a read is 0 while bytes or a writer remain, EOF with neither.
    #[test]
    fn zero_length_calls() {
        let mut p = P::new(4);
        assert_eq!(p.write(b""), (WriteOutcome::Written(0), NONE));
        assert_eq!(p.read(0).0, ReadOutcome::Read(0));
        p.write(b"x");
        assert_eq!(p.read(0).0, ReadOutcome::Read(0));
        assert_eq!(p.pipe.buffered(), 1);
        p.write(b"yzw"); // full
        assert_eq!(p.write(b""), (WriteOutcome::Written(0), NONE));
        let mut no_reader = P::new(4);
        no_reader.pipe.close_endpoint(End::Read).unwrap();
        assert_eq!(no_reader.write(b"").0, WriteOutcome::BrokenPipe);
        let mut no_writer = P::new(4);
        no_writer.pipe.close_endpoint(End::Write).unwrap();
        assert_eq!(no_writer.read(0).0, ReadOutcome::Eof);
    }

    /// A long stream through a small pipe, in uneven pieces, arrives whole
    /// and in order, and every block is matched by a wake.
    #[test]
    fn a_long_stream_through_a_small_pipe() {
        let mut p = P::new(7);
        let input: Vec<u8> = (0..10_000u32).map(|i| (i * 31 % 251) as u8).collect();
        let mut sent = 0;
        let mut out = Vec::new();
        let mut writer_parked = false;
        let mut step = 0usize;
        while out.len() < input.len() {
            step += 1;
            if sent < input.len() && !writer_parked {
                let piece = &input[sent..(sent + step % 11 + 1).min(input.len())];
                match p.write(piece).0 {
                    WriteOutcome::Written(n) => sent += n,
                    WriteOutcome::WouldBlock => writer_parked = true,
                    WriteOutcome::BrokenPipe => unreachable!(),
                }
            }
            let (o, w, got) = p.read(step % 5 + 1);
            if let ReadOutcome::Read(_) = o {
                out.extend(got);
            }
            if let ReadOutcome::Read(n) = o {
                // Every read that freed space wakes a parked writer.
                assert_eq!(w.writers, n > 0);
            }
            if w.writers {
                writer_parked = false;
            }
        }
        assert_eq!(out, input);
    }

    // ---- L1a.1 step 2b: the stdio word and SYS_STDIO_INFO (THINK
    // `docs/THINK-2026-11-08-l1a1-stdio-selection.md` §2.2 and §2.4). The
    // expected values are written out by hand from those sections. ----

    fn sel(stdin: Option<u8>, stdout: Option<u8>, term: Option<u8>) -> StdioSel {
        StdioSel {
            stdin,
            stdout,
            term,
        }
    }

    /// The words the THINK and the scenario use, decoded.
    #[test]
    fn stdio_word_decodes_the_designed_words() {
        // Leg 1: stdin 0, stdout 1, no term (0x_FF_01_00).
        assert_eq!(
            decode_stdio_word(0x00FF_0100, 2, false),
            Some(sel(Some(0), Some(1), None))
        );
        // The probe's: stdin 0, stdout 1, term 2 (0x02_01_00).
        assert_eq!(
            decode_stdio_word(0x0002_0100, 3, true),
            Some(sel(Some(0), Some(1), Some(2)))
        );
        // The shell's child: channel 0, stdin 1, stdout 2 (0x_FF_02_01).
        assert_eq!(
            decode_stdio_word(0x00FF_0201, 3, false),
            Some(sel(Some(1), Some(2), None))
        );
        // All none is SYS_SPAWN's child, even with no handles.
        assert_eq!(
            decode_stdio_word(0x00FF_FFFF, 0, false),
            Some(StdioSel::NONE)
        );
        assert!(StdioSel::NONE.is_none());
        assert!(!sel(None, Some(0), None).is_none());
    }

    /// Every step-2 refusal (THINK §2.2, "The errors, in order", step 2).
    #[test]
    fn stdio_word_refusals() {
        // A bit in 24-63: bit 24, bit 63.
        assert_eq!(decode_stdio_word(0x01FF_FFFF, 4, true), None);
        assert_eq!(decode_stdio_word(0x8000_0000_00FF_FFFF, 4, true), None);
        // An index at or past n (n = 2: index 2 is out; 0xFE is not 0xFF).
        assert_eq!(decode_stdio_word(0x00FF_0200, 2, true), None);
        assert_eq!(decode_stdio_word(0x00FF_FFFE, 2, true), None);
        // n = 0 with any index.
        assert_eq!(decode_stdio_word(0x00FF_FF00, 0, true), None);
        // Two roles on one index: stdin = stdout, stdin = term, stdout = term.
        assert_eq!(decode_stdio_word(0x00FF_0000, 2, true), None);
        assert_eq!(decode_stdio_word(0x0000_FF00, 2, true), None);
        assert_eq!(decode_stdio_word(0x0001_01FF, 2, true), None);
        // A term index in a kernel without buffers.
        assert_eq!(decode_stdio_word(0x0002_0100, 3, false), None);
        // The same word is fine where buffers exist.
        assert!(decode_stdio_word(0x0002_0100, 3, true).is_some());
    }

    /// `SYS_STDIO_INFO`'s byte oracle: THINK §2.4's four rows, hand-laid,
    /// against the kernel's encoder and libneo's decoder.
    #[test]
    fn stdio_info_oracle() {
        use super::libneo_pipe_abi as user;
        // (bytes, (stdin, stdout, term, selected))
        type Row = ([u8; 8], (Option<u16>, Option<u16>, Option<u16>, bool));
        let rows: [Row; 4] = [
            // stdin 0, stdout 1, term 2, selected.
            (
                [0x00, 0x00, 0x01, 0x00, 0x02, 0x00, 0x01, 0x00],
                (Some(0), Some(1), Some(2), true),
            ),
            // stdin 0, stdout 1, no term, selected.
            (
                [0x00, 0x00, 0x01, 0x00, 0xff, 0xff, 0x01, 0x00],
                (Some(0), Some(1), None, true),
            ),
            // selected, stdout since closed.
            (
                [0x00, 0x00, 0xff, 0xff, 0x02, 0x00, 0x01, 0x00],
                (Some(0), None, Some(2), true),
            ),
            // no selection, or all three none.
            (
                [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00],
                (None, None, None, false),
            ),
        ];
        for (bytes, (i, o, t, selected)) in rows {
            assert_eq!(encode_stdio_info(i, o, t, selected), bytes);
            let d = user::decode_stdio_info(bytes);
            assert_eq!(
                (d.stdin, d.stdout, d.term, d.selected),
                (
                    i.map(u32::from),
                    o.map(u32::from),
                    t.map(u32::from),
                    selected
                )
            );
        }
        assert_eq!(STDIO_INFO_LEN, 8);
        // A slot above 0xFF is still two bytes, little-endian.
        assert_eq!(
            encode_stdio_info(Some(0x0102), None, None, true),
            [0x02, 0x01, 0xff, 0xff, 0xff, 0xff, 0x01, 0x00]
        );
    }

    /// libneo's stdio word and syscall numbers against the kernel decoder
    /// and hand-written values.
    #[test]
    fn stdio_word_libneo_half() {
        use super::libneo_pipe_abi as user;
        assert_eq!(user::stdio_word(Some(0), Some(1), None), 0x00FF_0100);
        assert_eq!(user::stdio_word(Some(0), Some(1), Some(2)), 0x0002_0100);
        assert_eq!(user::stdio_word(Some(1), Some(2), None), 0x00FF_0201);
        assert_eq!(user::stdio_word(None, None, None), 0x00FF_FFFF);
        assert_eq!(user::STDIO_NO_INDEX, STDIO_NO_INDEX);
        assert_eq!(
            (
                user::SYS_SPAWN_STDIO,
                user::SYS_STDIO_INFO,
                user::SYS_STDIO_TERM_MAP
            ),
            (0x1BD, 0x1BE, 0x1BF)
        );
        for w in [
            user::stdio_word(Some(0), Some(1), None),
            user::stdio_word(Some(1), Some(2), None),
            user::stdio_word(Some(2), Some(0), Some(1)),
        ] {
            assert!(decode_stdio_word(w, 3, true).is_some(), "{w:#x}");
        }
    }
}
