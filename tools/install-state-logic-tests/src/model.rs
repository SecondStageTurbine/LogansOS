// SPDX-License-Identifier: MPL-2.0
//! A write-back-cache disk (THINK §4.7, "Tests"). Submitted writes sit in a
//! volatile cache until a flush; reads see them. Power can be cut at any
//! event (a submission or a flush): then any subset of the unflushed writes
//! reaches the media, sector by sector, in any order, and the device is gone.
//! A flush can also fail: it persists any subset of what it covered and
//! reports failure, after which the installer must submit nothing more.

use std::collections::BTreeMap;

use crate::gpt_logic::Disk;
use crate::install_logic::Target;

/// How the unflushed sectors land when power is cut or a flush fails.
#[derive(Clone, Copy, Debug)]
pub enum Land {
    /// Exactly the pending write calls whose index (in submission order,
    /// since the last flush) has its bit set; each call lands whole.
    Calls(u64),
    /// Sector by sector, a pseudo-random subset, the overlapping ones in a
    /// pseudo-random order.
    Seeded(u64),
}

/// What happens at event `at` (0-based, counting submissions and flushes).
#[derive(Clone, Copy, Debug)]
pub enum Fault {
    None,
    /// Power is cut just before event `at` executes.
    Cut {
        at: usize,
        land: Land,
    },
    /// Event `at`, which must be a flush, fails after landing `land`.
    FlushFails {
        at: usize,
        land: Land,
    },
}

pub struct Model {
    pub sector: u64,
    pub capacity: u64,
    /// Durable sectors; absent means zero.
    pub media: BTreeMap<u64, Vec<u8>>,
    /// Unflushed write calls, in order: each a run of sectors.
    pending: Vec<Vec<(u64, Vec<u8>)>>,
    pub events: usize,
    fault: Fault,
    /// The power is gone (after a cut).
    pub dead: bool,
    /// A flush failed; any later submission is a violation of rule 4.
    pub flush_failed: bool,
    pub wrote_after_failed_flush: bool,
    /// Boundary checks the installer made.
    pub boundaries: usize,
    /// Every header write: (LBA, whether the entry array it names was
    /// durable at the moment it was submitted: every array sector's latest
    /// written bytes already on the media).
    pub header_submissions: Vec<(u64, bool)>,
    /// The array each header names: (header LBA, array LBA, array sectors).
    pub header_arrays: Vec<(u64, u64, u64)>,
}

/// xorshift64*: deterministic choices for [`Land::Seeded`].
fn next(state: &mut u64) -> u64 {
    *state ^= *state >> 12;
    *state ^= *state << 25;
    *state ^= *state >> 27;
    state.wrapping_mul(0x2545_f491_4f6c_dd1d)
}

impl Model {
    pub fn new(sector: u64, capacity: u64, fault: Fault) -> Self {
        Self {
            sector,
            capacity,
            media: BTreeMap::new(),
            pending: Vec::new(),
            events: 0,
            fault,
            dead: false,
            flush_failed: false,
            wrote_after_failed_flush: false,
            boundaries: 0,
            header_submissions: Vec::new(),
            header_arrays: Vec::new(),
        }
    }

    /// A model over `media` (the disk after an earlier session), no faults
    /// unless given.
    pub fn resume(sector: u64, capacity: u64, media: BTreeMap<u64, Vec<u8>>, fault: Fault) -> Self {
        let mut m = Self::new(sector, capacity, fault);
        m.media = media;
        m
    }

    fn sector_bytes(&self, lba: u64) -> Vec<u8> {
        for call in self.pending.iter().rev() {
            if let Some((_, b)) = call.iter().rev().find(|(l, _)| *l == lba) {
                return b.clone();
            }
        }
        self.media
            .get(&lba)
            .cloned()
            .unwrap_or_else(|| vec![0; self.sector as usize])
    }

    fn durable_bytes(&self, lba: u64) -> Vec<u8> {
        self.media
            .get(&lba)
            .cloned()
            .unwrap_or_else(|| vec![0; self.sector as usize])
    }

    /// Land the pending writes as `land` says, then forget them.
    fn land(&mut self, land: Land) {
        let pending = std::mem::take(&mut self.pending);
        match land {
            Land::Calls(mask) => {
                for (i, call) in pending.into_iter().enumerate() {
                    if i < 64 && mask & (1 << i) != 0 {
                        for (lba, b) in call {
                            self.media.insert(lba, b);
                        }
                    }
                }
            }
            Land::Seeded(seed) => {
                let mut state = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1;
                let mut sectors: Vec<(u64, Vec<u8>)> = pending.into_iter().flatten().collect();
                // Fisher-Yates: any order.
                for i in (1..sectors.len()).rev() {
                    let j = (next(&mut state) % (i as u64 + 1)) as usize;
                    sectors.swap(i, j);
                }
                for (lba, b) in sectors {
                    if next(&mut state) & 1 == 1 {
                        self.media.insert(lba, b);
                    }
                }
            }
        }
    }

    /// Count an event; true if it may execute.
    fn event(&mut self, is_flush: bool) -> Result<(), bool> {
        if self.dead {
            return Err(false);
        }
        let at = self.events;
        self.events += 1;
        match self.fault {
            Fault::Cut { at: cut, land } if cut == at => {
                self.land(land);
                self.dead = true;
                Err(false)
            }
            Fault::FlushFails { at: fail, land } if fail == at => {
                assert!(is_flush, "event {at} is not a flush");
                self.land(land);
                self.flush_failed = true;
                Err(false)
            }
            _ => Ok(()),
        }
    }

    pub fn image(&self) -> Image<'_> {
        Image(self)
    }
}

impl Target for Model {
    fn sector_size(&self) -> u64 {
        self.sector
    }

    fn write(&mut self, lba: u64, bytes: &[u8]) -> bool {
        assert!((bytes.len() as u64).is_multiple_of(self.sector) && !bytes.is_empty());
        if self.flush_failed {
            self.wrote_after_failed_flush = true;
        }
        if let Some((_, array_lba, sectors)) = self
            .header_arrays
            .iter()
            .find(|(h, _, _)| *h == lba)
            .copied()
        {
            let durable = (0..sectors)
                .all(|i| self.durable_bytes(array_lba + i) == self.sector_bytes(array_lba + i));
            self.header_submissions.push((lba, durable));
        }
        if self.event(false).is_err() {
            return false;
        }
        let s = self.sector as usize;
        let call = bytes
            .chunks(s)
            .enumerate()
            .map(|(i, c)| (lba + i as u64, c.to_vec()))
            .collect();
        self.pending.push(call);
        true
    }

    fn flush(&mut self) -> bool {
        if self.event(true).is_err() {
            return false;
        }
        for (lba, b) in std::mem::take(&mut self.pending).into_iter().flatten() {
            self.media.insert(lba, b);
        }
        true
    }

    fn read(&mut self, lba: u64, out: &mut [u8]) -> bool {
        if self.dead {
            return false;
        }
        for (i, chunk) in out.chunks_mut(self.sector as usize).enumerate() {
            chunk.copy_from_slice(&self.sector_bytes(lba + i as u64)[..chunk.len()]);
        }
        true
    }

    fn boundary(&mut self) -> bool {
        self.boundaries += 1;
        !self.dead
    }
}

/// The durable media as a [`Disk`]: what the next boot reads.
pub struct Image<'a>(&'a Model);

impl Disk for Image<'_> {
    fn capacity(&self) -> u64 {
        self.0.capacity
    }

    fn sector_size(&self) -> u64 {
        self.0.sector
    }

    fn read(&self, offset: u64, out: &mut [u8]) -> bool {
        if offset + out.len() as u64 > self.0.capacity {
            return false;
        }
        let s = self.0.sector;
        let mut done = 0usize;
        while done < out.len() {
            let at = offset + done as u64;
            let (lba, within) = (at / s, (at % s) as usize);
            let take = (s as usize - within).min(out.len() - done);
            match self.0.media.get(&lba) {
                Some(sector) => {
                    out[done..done + take].copy_from_slice(&sector[within..within + take])
                }
                None => out[done..done + take].fill(0),
            }
            done += take;
        }
        true
    }
}
