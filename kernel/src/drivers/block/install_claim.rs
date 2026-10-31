// SPDX-License-Identifier: MPL-2.0
//! OT-87 R4.1: the kernel half of the install claim
//! (`docs/THINK-2026-10-27-ot87-installer.md` §2a; the rules are in
//! `claim_logic`).
//!
//! - [`enumerate`] hands out one [`Enumerated`] per registered device: a
//!   `(slot, generation)` token, the driver's identity, the two sampled
//!   windows and the GPT disk GUID.
//! - [`claim`] turns one into a [`ClaimedWriter`], or refuses before any
//!   write. The writer owns the device object (`Arc<GuardedDevice>`), so a
//!   registry change cannot redirect it, and it is the only path past the
//!   write guard (`GuardedDevice::claimed_write`, gated by the device's
//!   `claimed` flag, which the writer clears when it is dropped).
//!
//! Every call issues block I/O and may park: process context, interrupts on.

extern crate alloc;

use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::Ordering;

use super::claim_logic::{
    self, ClaimContext, ClaimKind, ClaimTarget, DriverIdentity, Enumerated, Fingerprint, Refusal,
    Token, Windows,
};
use super::{BlockDevice, DmaBuffer, GuardReader, GuardedDevice, REGISTRY, gpt_logic};
use crate::serial_println;

/// The protected-disk table (THINK §2, layer 5): serials never offered as a
/// target, with no override. Empty until a lane needs one (a VMD lane).
pub const PROTECTED_SERIALS: &[&str] = &[];

/// The largest request the claim submits: 64 KiB, as the guard's reader.
const CHUNK: usize = 64 * 1024;

/// A registered device as a claim target. Constructed only here, and only
/// handed to `claim_logic::Claim`, which never gives it out.
struct KernelTarget {
    device: Arc<GuardedDevice>,
}

impl KernelTarget {
    fn sector(&self) -> u64 {
        self.device.inner.geometry().logical_block_size as u64
    }
}

impl ClaimTarget for KernelTarget {
    fn media_generation(&self) -> u64 {
        self.device.media_state().generation
    }

    fn alive(&self) -> bool {
        self.device.media_state().alive
    }

    fn identity(&self) -> Option<DriverIdentity> {
        self.device.driver_identity()
    }

    fn capacity(&self) -> u64 {
        let g = self.device.inner.geometry();
        g.block_count.saturating_mul(g.logical_block_size as u64)
    }

    /// Whole sectors, in requests of at most [`CHUNK`]. The media state is
    /// checked again between requests, so a change stops the write after the
    /// request it happened during (the claim's own post-check then stops the
    /// claim).
    fn write(&self, offset: u64, bytes: &[u8]) -> bool {
        let ss = self.sector();
        if ss == 0 || !offset.is_multiple_of(ss) || !(bytes.len() as u64).is_multiple_of(ss) {
            return false;
        }
        let start = self.device.media_state();
        let mut at = 0usize;
        while at < bytes.len() {
            let take = (bytes.len() - at).min(CHUNK);
            let Ok(buffer) = DmaBuffer::alloc_zeroed(take) else {
                return false;
            };
            // SAFETY: the buffer was just allocated and is not shared with any
            // in-flight request.
            unsafe { buffer.as_mut_slice() }.copy_from_slice(&bytes[at..at + take]);
            let lba = (offset + at as u64) / ss;
            if self
                .device
                .claimed_write(lba, core::slice::from_ref(&buffer))
                .is_err()
            {
                return false;
            }
            at += take;
            if at < bytes.len() && self.device.media_state() != start {
                return false;
            }
        }
        true
    }

    fn read(&self, offset: u64, out: &mut [u8]) -> bool {
        gpt_logic::Disk::read(&GuardReader(self.device.inner.as_ref()), offset, out)
    }

    fn flush(&self) -> bool {
        self.device.flush().is_ok()
    }
}

/// A live install claim (THINK §2a item 4): writes, flushes and step-boundary
/// checks under the claim's rules. Dropping it releases the device.
pub struct ClaimedWriter {
    claim: claim_logic::Claim<KernelTarget>,
    device: Arc<GuardedDevice>,
    slot: usize,
}

impl ClaimedWriter {
    pub fn kind(&self) -> ClaimKind {
        self.claim.kind()
    }

    pub fn slot(&self) -> usize {
        self.slot
    }

    /// OT-87 R4.3: the claimed device's geometry, flush support and
    /// identity, for the installer's checks and its NeoFS view.
    pub fn geometry(&self) -> super::BlockGeometry {
        self.device.inner.geometry()
    }

    pub fn supports_flush(&self) -> bool {
        self.device.inner.supports_flush()
    }

    pub fn identity(&self) -> super::BlockDeviceIdentity {
        self.device.inner.identity().clone()
    }

    pub fn stopped(&self) -> Option<Refusal> {
        self.claim.stopped()
    }

    pub fn expected(&self) -> &Windows {
        self.claim.expected()
    }

    pub fn write(&mut self, offset: u64, bytes: &[u8]) -> Result<(), Refusal> {
        let r = self.claim.write(offset, bytes);
        self.log_stop(&r);
        r
    }

    pub fn flush(&mut self) -> Result<(), Refusal> {
        let r = self.claim.flush();
        self.log_stop(&r);
        r
    }

    pub fn read(&mut self, offset: u64, out: &mut [u8]) -> Result<(), Refusal> {
        self.claim.read(offset, out)
    }

    pub fn boundary(&mut self) -> Result<(), Refusal> {
        let r = self.claim.boundary();
        self.log_stop(&r);
        r
    }

    fn log_stop(&self, r: &Result<(), Refusal>) {
        if let Err(why) = r
            && *why != Refusal::Stopped
        {
            serial_println!(
                "[ kernel ] install: claim stopped slot={} reason={:?}",
                self.slot,
                why
            );
        }
    }
}

impl Drop for ClaimedWriter {
    /// Releases the device. A claim that wrote anything has been used: the
    /// slot moves to a new generation, so the enumeration it came from is
    /// stale and claiming the disk again needs a fresh one, which re-reads
    /// everything (Muse's R4.1 review, item 1). A claim refused or dropped
    /// before its first write leaves the enumeration valid.
    fn drop(&mut self) {
        if self.claim.wrote() {
            let mut registry = REGISTRY.lock();
            if let Some((generation, _)) = registry.generations.get_mut(self.slot) {
                *generation += 1;
            }
        }
        self.device.claimed.store(false, Ordering::Release);
        serial_println!(
            "[ kernel ] install: claim released slot={} used={}",
            self.slot,
            self.claim.wrote()
        );
    }
}

/// The slot's generation now: a device whose driver declared it dead moves
/// its slot to a new generation, once (THINK §2a item 1).
fn current_generation(slot: usize) -> Option<(u64, Arc<GuardedDevice>)> {
    let device = REGISTRY.lock().guards.get(slot)?.clone();
    // Read outside the registry lock: a driver's `media_state` may take its
    // own queue lock (NVMe services its event queue there).
    let alive = device.media_state().alive;
    let mut registry = REGISTRY.lock();
    let (generation, counted) = registry.generations.get_mut(slot)?;
    if !alive && !*counted {
        *generation += 1;
        *counted = true;
    }
    Some((*generation, device))
}

/// OT-87 R4.4: what the disk in `slot` holds, read through the guard's
/// reader (reads only, no claim), for the installer's disk list. `None` when
/// the slot is empty.
pub fn class_of(slot: usize) -> Option<super::install_logic::Class> {
    let device = REGISTRY.lock().guards.get(slot)?.clone();
    Some(super::install_logic::classify(&GuardReader(
        device.inner.as_ref(),
    )))
}

/// OT-87 R4.4: the disk in `slot`'s capacity in bytes and logical sector
/// size, for the installer's disk list.
pub fn geometry(slot: usize) -> Option<(u64, u32)> {
    let device = REGISTRY.lock().guards.get(slot)?.clone();
    let g = device.inner.geometry();
    Some((
        g.block_count.saturating_mul(g.logical_block_size as u64),
        g.logical_block_size,
    ))
}

/// OT-87 R4.4: how many registered devices an install claim holds now. The
/// list takes and drops a claim per disk; this is 0 after it, whatever any
/// row's preflight refused.
pub fn claimed_count() -> usize {
    let guards = REGISTRY.lock().guards.clone();
    guards
        .iter()
        .filter(|d| d.claimed.load(Ordering::SeqCst))
        .count()
}

/// One row per registered device, for the installer's disk list.
pub fn enumerate() -> Vec<Enumerated> {
    let count = REGISTRY.lock().guards.len();
    let mut out = Vec::new();
    for slot in 0..count {
        let Some((generation, device)) = current_generation(slot) else {
            continue;
        };
        let target = KernelTarget {
            device: device.clone(),
        };
        let Some(fingerprint) = Fingerprint::read(&target) else {
            serial_println!("[ kernel ] install: enumerate slot={} unreadable", slot);
            continue;
        };
        let disk_guid =
            gpt_logic::read(&GuardReader(device.inner.as_ref())).map(|gpt| gpt.disk_guid);
        let identity = device.driver_identity();
        serial_println!(
            "[ kernel ] install: enumerate slot={} generation={} driver={} serial={:?} model={:?} identity_available={} capacity={} gpt={}",
            slot,
            generation,
            device.identity().driver,
            identity.as_ref().map(|i| i.serial.as_str()).unwrap_or(""),
            identity.as_ref().map(|i| i.model.as_str()).unwrap_or(""),
            identity.as_ref().is_some_and(DriverIdentity::available),
            target.capacity(),
            disk_guid.is_some()
        );
        out.push(Enumerated {
            token: Token { slot, generation },
            identity,
            fingerprint,
            disk_guid,
        });
    }
    out
}

/// Claim the device `enumerated` names, for `kind`. `boot_disk_guid` is the
/// boot medium's GPT disk GUID (Limine's executable-file response).
pub fn claim(
    enumerated: &Enumerated,
    kind: ClaimKind,
    boot_disk_guid: Option<[u8; 16]>,
) -> Result<ClaimedWriter, Refusal> {
    let slot = enumerated.token.slot;
    let Some((generation, device)) = current_generation(slot) else {
        return Err(Refusal::StaleToken);
    };
    let already_claimed = device.take_claim();
    let ctx = ClaimContext {
        slot_generation: generation,
        boot_disk_guid,
        protected: PROTECTED_SERIALS,
        already_claimed,
    };
    let target = KernelTarget {
        device: device.clone(),
    };
    let result = claim_logic::claim(target, enumerated, kind, &ctx);
    serial_println!(
        "[ kernel ] install: claim slot={} generation={} kind={:?} result={:?}",
        slot,
        generation,
        kind,
        result.as_ref().map(|_| ()).err()
    );
    match result {
        Ok(claim) => Ok(ClaimedWriter {
            claim,
            device,
            slot,
        }),
        Err(why) => {
            if !already_claimed {
                device.claimed.store(false, Ordering::Release);
            }
            Err(why)
        }
    }
}
