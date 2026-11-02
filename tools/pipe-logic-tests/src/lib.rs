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
        assert_eq!(p.read(3), (ReadOutcome::Read(3), NONE, b"hel".to_vec()));
        assert_eq!(
            p.read(100),
            (ReadOutcome::Read(8), NONE, b"lo world".to_vec())
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

    /// A write larger than the space stores what fits (partial success);
    /// a full pipe would block; draining a full pipe wakes the writers.
    #[test]
    fn partial_write_then_block_then_wake() {
        let mut p = P::new(4);
        assert_eq!(p.write(b"abcdef"), (WriteOutcome::Written(4), READERS));
        assert!(!p.pipe.writable());
        assert_eq!(p.write(b"g"), (WriteOutcome::WouldBlock, NONE));
        assert_eq!(p.read(1), (ReadOutcome::Read(1), WRITERS, b"a".to_vec()));
        assert!(p.pipe.writable());
        // Not full any more: a further read wakes nobody.
        assert_eq!(p.read(1).1, NONE);
    }

    /// OT-88 §3.2: "last writer closed → reader gets EOF after the buffer
    /// drains".
    #[test]
    fn eof_comes_after_the_buffer_drains() {
        let mut p = P::new(16);
        p.write(b"tail");
        assert_eq!(p.pipe.close_endpoint(End::Write), Ok(READERS));
        assert!(p.pipe.readable());
        assert_eq!(p.read(2), (ReadOutcome::Read(2), NONE, b"ta".to_vec()));
        assert_eq!(p.read(9), (ReadOutcome::Read(2), NONE, b"il".to_vec()));
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

    #[test]
    fn zero_length_calls() {
        let mut p = P::new(4);
        assert_eq!(p.write(b""), (WriteOutcome::Written(0), NONE));
        assert_eq!(p.read(0).0, ReadOutcome::Read(0));
        p.write(b"x");
        assert_eq!(p.read(0).0, ReadOutcome::Read(0));
        assert_eq!(p.pipe.buffered(), 1);
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
            if w.writers {
                writer_parked = false;
            }
            if writer_parked && !p.pipe.writable() {
                // Parked on a full pipe: the next read must wake it.
                let (_, w, got) = p.read(1);
                out.extend(got);
                assert!(w.writers, "a read from a full pipe wakes the writer");
                writer_parked = false;
            }
        }
        assert_eq!(out, input);
    }
}
