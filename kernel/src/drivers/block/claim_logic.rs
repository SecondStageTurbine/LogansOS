// SPDX-License-Identifier: MPL-2.0
//! OT-87 R4.1: device identity and the install claim
//! (`docs/THINK-2026-10-27-ot87-installer.md` §2a), pure.
//!
//! Host-tested by `tools/install-claim-logic-tests` through `#[path]`, so keep
//! it free of `crate::` paths and feature cfgs. The kernel implements
//! [`ClaimTarget`] for a registered `GuardedDevice`; the KATs implement it for a
//! model device that can be swapped, killed or bumped mid-request.
//!
//! What it decides:
//! - **Identity** (§2a item 2): the model and serial a driver reports (NVMe
//!   Identify Controller, ATA IDENTIFY DEVICE, virtio-blk `GET_ID`), and
//!   whether that counts as an identity at all ([`DriverIdentity::available`]).
//! - **Tokens** (item 1): an enumeration hands out `(slot, generation)`; a
//!   claim is refused when the slot's generation has moved on.
//! - **The claim** (items 4–6): one-shot, bound to the device object, `Fresh`
//!   or `Destructive`. A destructive claim needs an available identity.
//! - **Every write** (item 5): the device is alive and its `media_generation`
//!   equals the claim's before submission and again after completion. A
//!   mismatch stops the claim for good.
//! - **Every step boundary** (item 5): the identity re-reads equal to the
//!   snapshot, and the first and last MiB equal the expected bytes: the bytes
//!   seen at enumeration, overlaid with every byte the claim has written there
//!   (item 3).
//!
//! What it cannot decide (item 5, stated as a limit, and asserted by a KAT):
//! a replacement that the driver does not report, that reports the same
//! identity, and whose distinguishing bytes in the sampled windows the
//! current step overwrites, is not detected.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

/// The sampled windows: the first and the last MiB (§2a item 3).
pub const WINDOW_BYTES: u64 = 1024 * 1024;

/// What a driver reports about the medium behind it.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct DriverIdentity {
    /// Trimmed ASCII; empty when the device reports none.
    pub model: String,
    /// Trimmed ASCII; empty when the device reports none.
    pub serial: String,
    pub capacity_bytes: u64,
    pub sector_size: u32,
    /// NVMe: the namespace's NGUID then EUI64 (Identify Namespace bytes
    /// 104..128), so a twin namespace with the same serial, model and
    /// capacity is a different disk. `None` for a driver with no such field.
    pub namespace_ids: Option<[u8; 24]>,
}

impl DriverIdentity {
    /// Whether this identifies the device at all: a serial that is not empty
    /// after trimming and is printable ASCII (§2a item 2). A QEMU virtio disk
    /// without `serial=` reports none.
    ///
    /// An NVMe namespace that reports neither an NGUID nor an EUI64 (all 24
    /// bytes zero) is no identity either: it can't be told from a twin, so a
    /// destructive claim on it is refused (fail-closed). A fresh claim is
    /// still allowed, as for any disk without an identity.
    pub fn available(&self) -> bool {
        !self.serial.is_empty()
            && self
                .serial
                .bytes()
                .all(|b| (0x21..=0x7e).contains(&b) || b == b' ')
            && self.namespace_ids != Some([0; 24])
    }
}

/// `raw` as trimmed ASCII: NULs and spaces stripped from both ends. A
/// non-printable byte inside makes the whole field count as absent (an empty
/// string), so it can never pass as an identity.
pub fn ascii_field(raw: &[u8]) -> String {
    let start = raw.iter().position(|&b| b != 0 && b != b' ');
    let end = raw.iter().rposition(|&b| b != 0 && b != b' ');
    let (Some(start), Some(end)) = (start, end) else {
        return String::new();
    };
    let body = &raw[start..=end];
    if body.iter().any(|&b| !(0x20..=0x7e).contains(&b)) {
        return String::new();
    }
    body.iter().map(|&b| b as char).collect()
}

/// NVMe Identify Controller (CNS 01h, 4096 bytes; NVMe Base 2.0 Figure 275):
/// SN at bytes 4..24, MN at bytes 24..64, both ASCII, space-padded.
/// `(serial, model)`; empty strings when the buffer is too short.
pub fn nvme_serial_model(identify_controller: &[u8]) -> (String, String) {
    if identify_controller.len() < 64 {
        return (String::new(), String::new());
    }
    (
        ascii_field(&identify_controller[4..24]),
        ascii_field(&identify_controller[24..64]),
    )
}

/// ATA IDENTIFY DEVICE (512 bytes; ACS-4 §7.13.6): the serial in words 10–19
/// and the model in words 27–46. Each word stores its two characters
/// high byte first, so the bytes of every word are swapped.
pub fn ata_serial_model(identify: &[u8]) -> (String, String) {
    if identify.len() < 94 {
        return (String::new(), String::new());
    }
    let words = |first: usize, last: usize| -> Vec<u8> {
        let mut out = Vec::with_capacity((last + 1 - first) * 2);
        for w in first..=last {
            out.push(identify[2 * w + 1]);
            out.push(identify[2 * w]);
        }
        out
    };
    (ascii_field(&words(10, 19)), ascii_field(&words(27, 46)))
}

/// The capacity ATA IDENTIFY DEVICE reports (ACS-4 §7.13.6): sectors from
/// words 100–103 (48-bit), else words 60–61 (28-bit); the logical sector
/// size from words 117–118 (in words) when word 106 is valid (bits 15:14 =
/// 01b) and bit 12 says sectors are longer than 256 words, else 512.
/// `(sectors, sector_size)`; zeros when the buffer is too short.
pub fn ata_capacity(identify: &[u8]) -> (u64, u32) {
    if identify.len() < 238 {
        return (0, 0);
    }
    let word = |w: usize| u16::from_le_bytes([identify[2 * w], identify[2 * w + 1]]);
    let mut sectors = (0..4).fold(0u64, |acc, i| acc | (word(100 + i) as u64) << (16 * i));
    if sectors == 0 {
        sectors = word(60) as u64 | (word(61) as u64) << 16;
    }
    let w106 = word(106);
    let sector_size = if w106 & 0xc000 == 0x4000 && w106 & (1 << 12) != 0 {
        (word(117) as u32 | (word(118) as u32) << 16).saturating_mul(2)
    } else {
        512
    };
    (sectors, sector_size)
}

/// virtio-blk `VIRTIO_BLK_T_GET_ID` (virtio 1.2 §5.2.6): 20 bytes, the serial
/// as ASCII, NUL-padded (and not NUL-terminated when it is 20 long).
pub fn virtio_serial(get_id: &[u8]) -> String {
    ascii_field(&get_id[..get_id.len().min(20)])
}

/// An enumeration's handle on a registry slot (§2a item 1). Never a bare
/// index: the generation names which device occupied the slot then.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Token {
    pub slot: usize,
    pub generation: u64,
}

/// What kind of install the claim permits (§4.5).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ClaimKind {
    /// A blank or an INCOMPLETE disk: nothing on it to lose.
    Fresh,
    /// Erase a foreign disk, or reinstall over a completed LogansOS (R4b).
    Destructive,
}

/// Why a claim was refused, or a claimed write stopped.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Refusal {
    /// The slot's generation moved on since enumeration (a reorder, a
    /// re-registration, a removal).
    StaleToken,
    /// The driver declared the device dead.
    DeviceDead,
    /// A destructive claim on a disk that does not identify itself (item 6).
    IdentityUnavailable,
    /// The disk is the one LogansOS booted from (§2, layer 3).
    BootDisk,
    /// The disk is in the protected-disk table (§2, layer 5).
    Protected,
    /// The device already has a live claim.
    AlreadyClaimed,
    /// The windows read now differ from those seen at enumeration.
    ChangedSinceEnumeration,
    /// `media_generation` moved before a submission: nothing was submitted.
    MediaChangedBefore,
    /// `media_generation` moved during a request: it may have reached another
    /// medium. The claim stops.
    MediaChangedAfter,
    /// The identity re-read at a step boundary differs from the snapshot.
    IdentityChanged,
    /// The windows re-read at a step boundary differ from the expected bytes.
    FingerprintMismatch,
    /// The driver returned an error.
    Io,
    /// A write or read outside the device.
    OutOfRange,
    /// An earlier stop: the claim writes nothing more.
    Stopped,
}

/// What a claim needs from a device. The kernel's implementation wraps the
/// registered `GuardedDevice` (and writes past the guard, under the claim);
/// the KATs use a model device.
pub trait ClaimTarget {
    /// The driver's own counter, bumped on every event that can mean a
    /// different medium (§2a item 5).
    fn media_generation(&self) -> u64;
    /// False once the driver reported a fatal state.
    fn alive(&self) -> bool;
    /// The identity as the driver reads it now.
    fn identity(&self) -> Option<DriverIdentity>;
    fn capacity(&self) -> u64;
    /// Whole sectors at a sector-aligned byte offset.
    fn write(&self, offset: u64, bytes: &[u8]) -> bool;
    fn read(&self, offset: u64, out: &mut [u8]) -> bool;
    /// Make every completed write durable. False on an error.
    fn flush(&self) -> bool;
}

/// The first and the last MiB of a device, as they are expected to read.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Windows {
    pub capacity: u64,
    pub first: Vec<u8>,
    pub last: Vec<u8>,
}

/// The step at which windows are read and compared: no 1 MiB scratch
/// buffer on the kernel heap (4 MiB in most builds).
const WINDOW_CHUNK: usize = 64 * 1024;

/// Where the windows of a `capacity`-byte device start, and how long each is.
fn window_spans(capacity: u64) -> (usize, u64) {
    let len = WINDOW_BYTES.min(capacity);
    (len as usize, capacity - len)
}

/// Read `len` bytes at `offset` in [`WINDOW_CHUNK`] steps, handing each
/// step to `each` with its position in the window. `false` on a read error
/// or when `each` returns `false`.
fn read_window(
    target: &dyn ClaimTarget,
    offset: u64,
    len: usize,
    mut each: impl FnMut(usize, &[u8]) -> bool,
) -> bool {
    let mut buf = alloc::vec![0u8; WINDOW_CHUNK.min(len)];
    let mut at = 0usize;
    while at < len {
        let take = (len - at).min(WINDOW_CHUNK);
        if !target.read(offset + at as u64, &mut buf[..take]) || !each(at, &buf[..take]) {
            return false;
        }
        at += take;
    }
    true
}

/// What enumeration keeps of the windows: their BLAKE3 digests, so a disk
/// list costs 64 bytes a row instead of 2 MiB.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Fingerprint {
    pub capacity: u64,
    pub first: [u8; 32],
    pub last: [u8; 32],
}

impl Fingerprint {
    /// The digests of `target`'s two windows. `None` on a read error.
    pub fn read(target: &dyn ClaimTarget) -> Option<Self> {
        let capacity = target.capacity();
        let (len, last_start) = window_spans(capacity);
        let digest = |offset: u64| {
            let mut h = blake3::Hasher::new();
            read_window(target, offset, len, |_, chunk| {
                h.update(chunk);
                true
            })
            .then(|| *h.finalize().as_bytes())
        };
        Some(Self {
            capacity,
            first: digest(0)?,
            last: digest(last_start)?,
        })
    }
}

impl Windows {
    /// The two windows of `target` (each `WINDOW_BYTES`, or the whole device
    /// when it is smaller). `None` on a read error.
    pub fn read(target: &dyn ClaimTarget) -> Option<Self> {
        let capacity = target.capacity();
        let (len, last_start) = window_spans(capacity);
        let mut first = alloc::vec![0u8; len];
        let mut last = alloc::vec![0u8; len];
        if !target.read(0, &mut first) || !target.read(last_start, &mut last) {
            return None;
        }
        Some(Self {
            capacity,
            first,
            last,
        })
    }

    /// The digests of these windows.
    pub fn fingerprint(&self) -> Fingerprint {
        Fingerprint {
            capacity: self.capacity,
            first: *blake3::hash(&self.first).as_bytes(),
            last: *blake3::hash(&self.last).as_bytes(),
        }
    }

    /// Whether `target`'s windows read equal to these, compared a chunk at a
    /// time. `None` on a read error.
    pub fn matches(&self, target: &dyn ClaimTarget) -> Option<bool> {
        if target.capacity() != self.capacity {
            return Some(false);
        }
        let (len, last_start) = window_spans(self.capacity);
        for (offset, expected) in [(0, &self.first), (last_start, &self.last)] {
            let mut equal = true;
            let read = read_window(target, offset, len, |at, chunk| {
                equal = chunk == &expected[at..at + chunk.len()];
                equal
            });
            if !equal {
                return Some(false);
            }
            if !read {
                return None;
            }
        }
        Some(true)
    }

    /// Overlay a write of `bytes` at `offset` (item 3): the windows then
    /// expect what was written.
    pub fn overlay(&mut self, offset: u64, bytes: &[u8]) {
        let len = self.first.len() as u64;
        let last_start = self.capacity - len;
        for (i, &b) in bytes.iter().enumerate() {
            let at = offset + i as u64;
            if at < len {
                self.first[at as usize] = b;
            }
            if at >= last_start && at < self.capacity {
                self.last[(at - last_start) as usize] = b;
            }
        }
    }
}

/// What enumeration saw, and what the claim must be checked against.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Enumerated {
    pub token: Token,
    pub identity: Option<DriverIdentity>,
    pub fingerprint: Fingerprint,
    /// GPT disk GUID, when the disk has a valid GPT.
    pub disk_guid: Option<[u8; 16]>,
}

/// The context a claim is decided in.
pub struct ClaimContext<'a> {
    /// The slot's generation now.
    pub slot_generation: u64,
    /// The boot medium's GPT disk GUID (Limine's executable-file response).
    pub boot_disk_guid: Option<[u8; 16]>,
    /// The protected-disk table: identities never offered (§2, layer 5).
    pub protected: &'a [&'a str],
    /// Whether the device already has a live claim.
    pub already_claimed: bool,
}

/// A live, one-shot claim on one device object (§2a item 4).
pub struct Claim<T: ClaimTarget> {
    target: T,
    kind: ClaimKind,
    media_generation: u64,
    identity: Option<DriverIdentity>,
    expected: Windows,
    stopped: Option<Refusal>,
    /// Whether any write was submitted: a claim that wrote has been used, and
    /// its enumeration expires with it (Muse's R4.1 review, item 1).
    wrote: bool,
}

/// Claim `target` (the device object behind `enumerated.token`) for `kind`.
/// Every refusal is decided before any write.
pub fn claim<T: ClaimTarget>(
    target: T,
    enumerated: &Enumerated,
    kind: ClaimKind,
    ctx: &ClaimContext<'_>,
) -> Result<Claim<T>, Refusal> {
    if ctx.already_claimed {
        return Err(Refusal::AlreadyClaimed);
    }
    if enumerated.token.generation != ctx.slot_generation {
        return Err(Refusal::StaleToken);
    }
    if !target.alive() {
        return Err(Refusal::DeviceDead);
    }
    if ctx.boot_disk_guid.is_some() && enumerated.disk_guid == ctx.boot_disk_guid {
        return Err(Refusal::BootDisk);
    }
    let identity = target.identity();
    if let Some(id) = identity.as_ref().filter(|id| id.available())
        && ctx.protected.iter().any(|p| *p == id.serial)
    {
        return Err(Refusal::Protected);
    }
    if kind == ClaimKind::Destructive && !identity.as_ref().is_some_and(DriverIdentity::available) {
        return Err(Refusal::IdentityUnavailable);
    }
    if identity != enumerated.identity {
        return Err(Refusal::IdentityChanged);
    }
    // The generation is read before the windows: a change during the read is
    // seen at the first write's pre-check.
    let media_generation = target.media_generation();
    let expected = match Windows::read(&target) {
        Some(now) if now.fingerprint() == enumerated.fingerprint => now,
        Some(_) => return Err(Refusal::ChangedSinceEnumeration),
        None => return Err(Refusal::Io),
    };
    Ok(Claim {
        target,
        kind,
        media_generation,
        identity,
        expected,
        stopped: None,
        wrote: false,
    })
}

impl<T: ClaimTarget> Claim<T> {
    pub fn kind(&self) -> ClaimKind {
        self.kind
    }

    /// Why the claim stopped, if it has.
    pub fn stopped(&self) -> Option<Refusal> {
        self.stopped
    }

    pub fn expected(&self) -> &Windows {
        &self.expected
    }

    /// Whether any write was submitted under this claim.
    pub fn wrote(&self) -> bool {
        self.wrote
    }

    fn stop(&mut self, why: Refusal) -> Refusal {
        self.stopped.get_or_insert(why);
        why
    }

    /// One claimed write (§2a item 5): alive and the claim's media generation
    /// before submission (else nothing is submitted), the same generation
    /// after completion (else the claim stops: the request may have reached
    /// another medium). On success the expected windows take the bytes.
    pub fn write(&mut self, offset: u64, bytes: &[u8]) -> Result<(), Refusal> {
        if self.stopped.is_some() {
            return Err(Refusal::Stopped);
        }
        let end = offset.checked_add(bytes.len() as u64);
        if end.is_none_or(|end| end > self.expected.capacity) {
            return Err(Refusal::OutOfRange);
        }
        if !self.target.alive() {
            return Err(self.stop(Refusal::DeviceDead));
        }
        if self.target.media_generation() != self.media_generation {
            return Err(self.stop(Refusal::MediaChangedBefore));
        }
        self.wrote = true;
        let ok = self.target.write(offset, bytes);
        if self.target.media_generation() != self.media_generation || !self.target.alive() {
            return Err(self.stop(Refusal::MediaChangedAfter));
        }
        if !ok {
            return Err(self.stop(Refusal::Io));
        }
        self.expected.overlay(offset, bytes);
        Ok(())
    }

    /// The step-boundary check (§2a item 5), after the step's flush: the
    /// identity re-reads equal to the snapshot, and both windows equal the
    /// expected bytes. A failure stops the claim.
    pub fn boundary(&mut self) -> Result<(), Refusal> {
        if self.stopped.is_some() {
            return Err(Refusal::Stopped);
        }
        if !self.target.alive() {
            return Err(self.stop(Refusal::DeviceDead));
        }
        if self.target.media_generation() != self.media_generation {
            return Err(self.stop(Refusal::MediaChangedBefore));
        }
        if self.target.identity() != self.identity {
            return Err(self.stop(Refusal::IdentityChanged));
        }
        match self.expected.matches(&self.target) {
            Some(true) => Ok(()),
            Some(false) => Err(self.stop(Refusal::FingerprintMismatch)),
            None => Err(self.stop(Refusal::Io)),
        }
    }

    /// A flush under the same before/after checks as a write.
    pub fn flush(&mut self) -> Result<(), Refusal> {
        if self.stopped.is_some() {
            return Err(Refusal::Stopped);
        }
        if !self.target.alive() {
            return Err(self.stop(Refusal::DeviceDead));
        }
        if self.target.media_generation() != self.media_generation {
            return Err(self.stop(Refusal::MediaChangedBefore));
        }
        let ok = self.target.flush();
        if self.target.media_generation() != self.media_generation || !self.target.alive() {
            return Err(self.stop(Refusal::MediaChangedAfter));
        }
        if !ok {
            return Err(self.stop(Refusal::Io));
        }
        Ok(())
    }

    /// A read (the installer's read-back verify). The target is never handed
    /// out, so nothing writes it except [`Claim::write`].
    pub fn read(&mut self, offset: u64, out: &mut [u8]) -> Result<(), Refusal> {
        if self.stopped.is_some() {
            return Err(Refusal::Stopped);
        }
        let end = offset.checked_add(out.len() as u64);
        if end.is_none_or(|end| end > self.expected.capacity) {
            return Err(Refusal::OutOfRange);
        }
        if self.target.read(offset, out) {
            Ok(())
        } else {
            Err(self.stop(Refusal::Io))
        }
    }
}

impl<T: ClaimTarget + ?Sized> ClaimTarget for &T {
    fn media_generation(&self) -> u64 {
        (**self).media_generation()
    }
    fn alive(&self) -> bool {
        (**self).alive()
    }
    fn identity(&self) -> Option<DriverIdentity> {
        (**self).identity()
    }
    fn capacity(&self) -> u64 {
        (**self).capacity()
    }
    fn write(&self, offset: u64, bytes: &[u8]) -> bool {
        (**self).write(offset, bytes)
    }
    fn read(&self, offset: u64, out: &mut [u8]) -> bool {
        (**self).read(offset, out)
    }
    fn flush(&self) -> bool {
        (**self).flush()
    }
}
