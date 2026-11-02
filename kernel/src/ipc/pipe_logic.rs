// SPDX-License-Identifier: MPL-2.0
//! The kernel pipe's pure state machine (Lane 1, rung L1a.1; OT-88 §3.2 T1,
//! `docs/THINK-2026-10-29-ot88-app-model.md`, and the terminal's use of it,
//! `docs/THINK-2026-10-31-l1a-terminal-window.md` §3.3).
//!
//! Shared by the kernel (behind feature `pipe`, wired in a later step) and
//! the host KATs in `tools/pipe-logic-tests`, which mount this file by
//! `#[path]` so they run this exact code. No locks, no allocation, no
//! scheduler here: the kernel holds the pipe's lock around each call, and
//! each call returns, with its outcome, the wakes it requires, so the check
//! and its effect are one critical section (CP-14: no lost wake between "it
//! was empty" and "the reader parked").
//!
//! The semantics, a bounded unframed byte stream:
//! - **write** stores as many bytes as fit (a partial write is a success);
//!   with no space it would block; with no reader left it is EPIPE at once.
//! - **read** returns what is buffered, up to the caller's length; with
//!   nothing buffered it would block while a writer remains, and is EOF
//!   once the last writer has closed (after the buffer drains).
//! - **Endpoints are counted per handle**, not per process: a granted end
//!   is one more endpoint; closing the original leaves the granted one live.
//! - The last reader closing wakes every parked writer (they get EPIPE);
//!   the last writer closing wakes every parked reader (they get EOF).

/// The two pipe sizes the terminal uses (L1a THINK §3.3).
pub const STDIN_PIPE_CAPACITY: usize = 4 * 1024;
pub const STDOUT_PIPE_CAPACITY: usize = 64 * 1024;
/// The largest pipe a caller may create.
pub const MAX_PIPE_CAPACITY: usize = 64 * 1024;
/// A write of at most this many bytes (and no more than the capacity) is
/// atomic: it is stored whole or waits for room, so two writers' small
/// writes never interleave (POSIX's minimum `PIPE_BUF`; step-2 THINK §2a).
pub const PIPE_ATOMIC: usize = 512;

/// Which end a handle is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    Read,
    Write,
}

/// Whom the caller must wake after the call (still under the pipe's lock,
/// or with the wait queues' own ordering).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Wake {
    pub readers: bool,
    pub writers: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteOutcome {
    /// `n` bytes stored: less than asked when the pipe fills; 0 only for an
    /// empty write.
    Written(usize),
    /// No space and a reader remains: park (or EAGAIN for a try).
    WouldBlock,
    /// No reader remains: EPIPE.
    BrokenPipe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadOutcome {
    /// `n` bytes copied out; 0 only for a zero-length read.
    Read(usize),
    /// Nothing buffered and a writer remains: park (or EAGAIN for a try).
    WouldBlock,
    /// Nothing buffered and no writer remains.
    Eof,
}

/// Why an endpoint can't be added or created.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PipeError {
    /// Capacity 0, or above [`MAX_PIPE_CAPACITY`], or storage of the wrong
    /// length.
    BadCapacity,
    /// The endpoint count would overflow.
    TooManyEndpoints,
    /// Closing an end that has no endpoints left (a double close).
    NoSuchEndpoint,
    /// Adding an endpoint to an end whose count reached 0: once an end is
    /// closed it stays closed (Dexter #46 note 1b).
    EndClosed,
}

/// The state of one pipe. The bytes live in caller-owned storage of exactly
/// `capacity` bytes, passed to each call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pipe {
    capacity: usize,
    head: usize,
    len: usize,
    readers: u32,
    writers: u32,
}

impl Pipe {
    /// A new pipe with one read end and one write end.
    pub const fn new(capacity: usize) -> Result<Self, PipeError> {
        if capacity == 0 || capacity > MAX_PIPE_CAPACITY {
            return Err(PipeError::BadCapacity);
        }
        Ok(Self {
            capacity,
            head: 0,
            len: 0,
            readers: 1,
            writers: 1,
        })
    }

    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    pub const fn buffered(&self) -> usize {
        self.len
    }

    pub const fn readers(&self) -> u32 {
        self.readers
    }

    pub const fn writers(&self) -> u32 {
        self.writers
    }

    /// Both ends are gone: the object (and its charge) can be freed.
    pub const fn is_dead(&self) -> bool {
        self.readers == 0 && self.writers == 0
    }

    /// A read would not block (bytes, or EOF): `SYS_WAIT_ANY`'s readiness
    /// for a read end.
    pub const fn readable(&self) -> bool {
        self.len > 0 || self.writers == 0
    }

    /// A write would not block (space, or EPIPE): readiness for a write end.
    pub const fn writable(&self) -> bool {
        self.len < self.capacity || self.readers == 0
    }

    /// A pipe with given endpoint counts, for the host KATs of the count
    /// bounds (`TooManyEndpoints`, `EndClosed`); the kernel uses [`Pipe::new`].
    #[doc(hidden)]
    pub const fn with_endpoints(
        capacity: usize,
        readers: u32,
        writers: u32,
    ) -> Result<Self, PipeError> {
        match Self::new(capacity) {
            Ok(mut p) => {
                p.readers = readers;
                p.writers = writers;
                Ok(p)
            }
            Err(e) => Err(e),
        }
    }

    /// One more handle on `end` (a grant or a duplicate). An end already
    /// closed (count 0) can't be reopened.
    pub fn add_endpoint(&mut self, end: End) -> Result<(), PipeError> {
        let count = match end {
            End::Read => &mut self.readers,
            End::Write => &mut self.writers,
        };
        if *count == 0 {
            return Err(PipeError::EndClosed);
        }
        *count = count.checked_add(1).ok_or(PipeError::TooManyEndpoints)?;
        Ok(())
    }

    /// A handle on `end` closed (an explicit close, or its process's
    /// teardown). The last one of an end wakes everyone parked on the other.
    pub fn close_endpoint(&mut self, end: End) -> Result<Wake, PipeError> {
        let count = match end {
            End::Read => &mut self.readers,
            End::Write => &mut self.writers,
        };
        *count = count.checked_sub(1).ok_or(PipeError::NoSuchEndpoint)?;
        let last = *count == 0;
        Ok(match end {
            // The last reader: writers get EPIPE. The buffer is now
            // unreachable, so drop it (its bytes can never be read).
            End::Read if last => {
                self.head = 0;
                self.len = 0;
                Wake {
                    readers: false,
                    writers: true,
                }
            }
            // The last writer: readers drain what's left, then EOF.
            End::Write if last => Wake {
                readers: true,
                writers: false,
            },
            _ => Wake::default(),
        })
    }

    /// Store up to `src.len()` bytes into `storage` (`capacity` bytes). A
    /// write of at most [`PIPE_ATOMIC`] bytes that fits the capacity is
    /// atomic: all of it, or `WouldBlock` until there is room for all of it.
    /// A larger write stores what fits. A zero-length write is `BrokenPipe`
    /// with no reader, else `Written(0)` (the zero-length contract, as
    /// Linux's).
    pub fn write(
        &mut self,
        storage: &mut [u8],
        src: &[u8],
    ) -> Result<(WriteOutcome, Wake), PipeError> {
        if storage.len() != self.capacity {
            return Err(PipeError::BadCapacity);
        }
        if self.readers == 0 {
            return Ok((WriteOutcome::BrokenPipe, Wake::default()));
        }
        if src.is_empty() {
            return Ok((WriteOutcome::Written(0), Wake::default()));
        }
        let space = self.capacity - self.len;
        let atomic = src.len() <= PIPE_ATOMIC && src.len() <= self.capacity;
        if space == 0 || (atomic && space < src.len()) {
            return Ok((WriteOutcome::WouldBlock, Wake::default()));
        }
        let n = space.min(src.len());
        let was_empty = self.len == 0;
        let tail = (self.head + self.len) % self.capacity;
        let first = n.min(self.capacity - tail);
        storage[tail..tail + first].copy_from_slice(&src[..first]);
        storage[..n - first].copy_from_slice(&src[first..n]);
        self.len += n;
        Ok((
            WriteOutcome::Written(n),
            Wake {
                readers: was_empty,
                writers: false,
            },
        ))
    }

    /// Copy up to `dst.len()` buffered bytes out of `storage`. A zero-length
    /// read is `Read(0)` while bytes or a writer remain, `Eof` with neither.
    ///
    /// Every read that frees space wakes the writers, not only one from a
    /// full pipe: an atomic writer parks while the pipe is not full but has
    /// too little room for its whole write, and must see each new byte of
    /// room. A writer that still doesn't fit parks again.
    pub fn read(
        &mut self,
        storage: &[u8],
        dst: &mut [u8],
    ) -> Result<(ReadOutcome, Wake), PipeError> {
        if storage.len() != self.capacity {
            return Err(PipeError::BadCapacity);
        }
        if self.len == 0 {
            let outcome = if self.writers == 0 {
                ReadOutcome::Eof
            } else if dst.is_empty() {
                ReadOutcome::Read(0)
            } else {
                ReadOutcome::WouldBlock
            };
            return Ok((outcome, Wake::default()));
        }
        if dst.is_empty() {
            return Ok((ReadOutcome::Read(0), Wake::default()));
        }
        let n = self.len.min(dst.len());
        let first = n.min(self.capacity - self.head);
        dst[..first].copy_from_slice(&storage[self.head..self.head + first]);
        dst[first..n].copy_from_slice(&storage[..n - first]);
        self.head = (self.head + n) % self.capacity;
        self.len -= n;
        if self.len == 0 {
            self.head = 0;
        }
        Ok((
            ReadOutcome::Read(n),
            Wake {
                readers: false,
                writers: true,
            },
        ))
    }
}
