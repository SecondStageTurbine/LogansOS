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
//!   The last close of an end also wakes the threads parked on that same
//!   end (a sibling closed the handle they were blocked on), and a read or
//!   write on a closed end is refused (`PipeError::EndClosed`, the kernel's
//!   `EBADF`): Dexter's #56 F1.

/// The two pipe sizes the terminal uses (L1a THINK §3.3).
pub const STDIN_PIPE_CAPACITY: usize = 4 * 1024;
pub const STDOUT_PIPE_CAPACITY: usize = 64 * 1024;
/// The largest pipe a caller may create.
pub const MAX_PIPE_CAPACITY: usize = 64 * 1024;
/// The smallest pipe `SYS_PIPE_CREATE` makes: a write of at most
/// [`PIPE_ATOMIC`] bytes is atomic only if it fits the capacity, so the
/// capacity must be at least that (POSIX: `PIPE_BUF` <= capacity; Chad #56
/// item 5). Kernel-internal pipes (the tests) may be smaller.
pub const MIN_PIPE_CAPACITY: usize = PIPE_ATOMIC;

/// `SYS_PIPE_CREATE`'s capacity rule: `MIN_PIPE_CAPACITY..=MAX_PIPE_CAPACITY`.
pub const fn create_capacity_ok(capacity: usize) -> bool {
    capacity >= MIN_PIPE_CAPACITY && capacity <= MAX_PIPE_CAPACITY
}

/// `SYS_PIPE_READ` / `SYS_PIPE_WRITE` flag: don't block (`EAGAIN`).
pub const PIPE_NONBLOCK: u64 = 1;

/// `SYS_PIPE_CREATE`'s return: the read end's handle in the low 32 bits,
/// the write end's in the high 32 (libneo splits it the same way; the byte
/// oracle is in `tools/pipe-logic-tests`).
pub const fn pack_ends(read: u32, write: u32) -> u64 {
    (read as u64) | ((write as u64) << 32)
}

/// The inverse of [`pack_ends`]: `(read, write)`.
pub const fn unpack_ends(packed: u64) -> (u32, u32) {
    (packed as u32, (packed >> 32) as u32)
}

/// L1a.1 step 2b (`docs/THINK-2026-11-08-l1a1-stdio-selection.md` §2.2): the
/// index byte in `SYS_SPAWN_STDIO`'s stdio word that means "no handle for
/// this role".
pub const STDIO_NO_INDEX: u8 = 0xFF;

/// A spawn's stdio selection: for each role, the index into the spawn's
/// `handles` array (which is also the child's slot), or `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StdioSel {
    pub stdin: Option<u8>,
    pub stdout: Option<u8>,
    pub term: Option<u8>,
}

impl StdioSel {
    /// No stdio selected: what `SYS_SPAWN` gives every child.
    pub const NONE: Self = Self {
        stdin: None,
        stdout: None,
        term: None,
    };

    pub const fn is_none(&self) -> bool {
        self.stdin.is_none() && self.stdout.is_none() && self.term.is_none()
    }
}

/// Decode `SYS_SPAWN_STDIO`'s stdio word for a spawn of `n` handles (THINK
/// §2.2, error step 2). Byte 0 is stdin's index, byte 1 stdout's, byte 2 the
/// term page's, `0xFF` for none. `None` (the syscall's `EINVAL`) when:
/// - a bit in 24-63 is set;
/// - an index is neither `0xFF` nor below `n`;
/// - two roles name the same index;
/// - a term index is given and `term_supported` is false (a kernel without
///   `atrium-compositor-g1`, which has no buffers).
///
/// All three none (`0x00FF_FFFF`) is [`StdioSel::NONE`], the same child as
/// `SYS_SPAWN`'s.
pub const fn decode_stdio_word(word: u64, n: usize, term_supported: bool) -> Option<StdioSel> {
    if word >> 24 != 0 {
        return None;
    }
    let bytes = [word as u8, (word >> 8) as u8, (word >> 16) as u8];
    let mut roles = [None; 3];
    let mut i = 0;
    while i < 3 {
        let b = bytes[i];
        if b != STDIO_NO_INDEX {
            if b as usize >= n {
                return None;
            }
            let mut j = 0;
            while j < i {
                if bytes[j] == b {
                    return None;
                }
                j += 1;
            }
            roles[i] = Some(b);
        }
        i += 1;
    }
    if roles[2].is_some() && !term_supported {
        return None;
    }
    Some(StdioSel {
        stdin: roles[0],
        stdout: roles[1],
        term: roles[2],
    })
}

/// `SYS_STDIO_INFO`'s record length (THINK §2.4).
pub const STDIO_INFO_LEN: usize = 8;
/// A slot field of the record that names no slot.
pub const STDIO_INFO_NO_SLOT: u16 = 0xFFFF;
/// The record's flags bit 0: a stdio was selected at spawn.
pub const STDIO_INFO_SELECTED: u16 = 1;

/// `SYS_STDIO_INFO`'s 8-byte record (THINK §2.4): the stdin, stdout and term
/// page slots as little-endian `u16` (`0xFFFF` for none, or for a slot that no
/// longer holds the selected handle), then the flags. The byte oracle is in
/// `tools/pipe-logic-tests` (`stdio_info_oracle`).
pub const fn encode_stdio_info(
    stdin: Option<u16>,
    stdout: Option<u16>,
    term: Option<u16>,
    selected: bool,
) -> [u8; STDIO_INFO_LEN] {
    const fn slot(s: Option<u16>) -> [u8; 2] {
        match s {
            Some(v) => v.to_le_bytes(),
            None => STDIO_INFO_NO_SLOT.to_le_bytes(),
        }
    }
    let (a, b, c) = (slot(stdin), slot(stdout), slot(term));
    let f = (if selected { STDIO_INFO_SELECTED } else { 0 }).to_le_bytes();
    [a[0], a[1], b[0], b[1], c[0], c[1], f[0], f[1]]
}
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
    /// Adding an endpoint to, or reading or writing through, an end whose
    /// count reached 0: once an end is closed it stays closed (Dexter #46
    /// note 1b; #56 F1, a thread whose sibling closed its handle).
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
    /// An end's count has dropped to 0: it is closed for good.
    read_closed: bool,
    write_closed: bool,
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
            read_closed: false,
            write_closed: false,
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

    /// A write of up to `PIPE_ATOMIC` bytes would not block (room for it
    /// whole, or EPIPE): readiness for a write end. "Any space" is not
    /// enough: an atomic write waits for room for all of it, so a wait on
    /// "any space" followed by a non-blocking write could spin (Dexter #56
    /// F2). With this much room every write makes progress: one of at most
    /// the threshold fits whole, a larger one is not atomic and stores some.
    pub const fn writable(&self) -> bool {
        let need = if self.capacity < PIPE_ATOMIC {
            self.capacity
        } else {
            PIPE_ATOMIC
        };
        self.capacity - self.len >= need || self.readers == 0
    }

    /// A new pipe with no endpoints yet, for a caller whose handle installs
    /// each count one (the kernel's `HandleTable` retains on install). Its
    /// ends are open, not closed: the first `add_endpoint` on each succeeds.
    pub const fn new_unattached(capacity: usize) -> Result<Self, PipeError> {
        match Self::new(capacity) {
            Ok(mut p) => {
                p.readers = 0;
                p.writers = 0;
                Ok(p)
            }
            Err(e) => Err(e),
        }
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

    /// One more handle on `end` (a grant or a duplicate). An end whose
    /// count has dropped to 0 can't be reopened.
    pub fn add_endpoint(&mut self, end: End) -> Result<(), PipeError> {
        let (count, closed) = match end {
            End::Read => (&mut self.readers, self.read_closed),
            End::Write => (&mut self.writers, self.write_closed),
        };
        if closed {
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
        if last {
            match end {
                End::Read => self.read_closed = true,
                End::Write => self.write_closed = true,
            }
        }
        if !last {
            return Ok(Wake::default());
        }
        // The last reader: the buffer is now unreachable, so drop it (its
        // bytes can never be read). The last writer: readers drain what's
        // left, then EOF.
        if end == End::Read {
            self.head = 0;
            self.len = 0;
        }
        // Wake both sides: the other side's waiters get EPIPE or EOF, and
        // this side's (a sibling closed the handle they were blocked on) get
        // `EndClosed` instead of sleeping on an end nothing can wake.
        Ok(Wake {
            readers: true,
            writers: true,
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
        if self.write_closed {
            return Err(PipeError::EndClosed);
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
        if self.read_closed {
            return Err(PipeError::EndClosed);
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
