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
        assert!(p.pipe.writable());
        // Not full any more, and the read still wakes the writers.
        assert_eq!(p.read(1).1, WRITERS);
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
        assert_eq!(p.pipe.close_endpoint(End::Write), Ok(READERS));
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
        assert_eq!(p.pipe.close_endpoint(End::Read), Ok(WRITERS));
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
        assert_eq!(p.pipe.close_endpoint(End::Write), Ok(READERS));
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
        assert_eq!(p.pipe.close_endpoint(End::Read), Ok(WRITERS));
        assert_eq!(p.write(b"c").0, WriteOutcome::BrokenPipe);
        assert_eq!(p.pipe.close_endpoint(End::Write), Ok(READERS));
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
}
