// SPDX-License-Identifier: MPL-2.0
//! Deterministic host fuzzing for the Lane 2 codecs (Lane 2c).
//!
//! Targets: zlib-wrapped inflate, raw DEFLATE inflate, the `neo-image-decode`
//! PNG path, and its JPEG path. The image targets call the same functions
//! `libneo::image_decode` re-exports.
//!
//! This is deliberately an **in-tree** fuzzer rather than `cargo-fuzz`. The
//! close condition is a fixed budget in ordinary CI, on the pinned nightly,
//! without the libFuzzer/sanitizer toolchain (`cargo-fuzz` is not installed and
//! the workspace does not otherwise need `-Z sanitizer`). The in-tree harness
//! also makes the budget, the seed and the mutation schedule explicit, so a
//! crash is reproducible from `(target, seed, iteration)` and a quarantined
//! input can be replayed with one command.
//!
//! Guards:
//!
//! - **Crash / panic** — a panic aborts the process (the workspace builds with
//!   `panic = "abort"`), so the CI job fails. The panic hook writes the exact
//!   input to `--crash-dir` before aborting.
//! - **Timeout** — the campaign stops itself at `--seconds`; the CI job wraps
//!   the process in `timeout(1)` as well, so a decoder that never returns still
//!   fails the job.
//! - **Allocation above the decode size cap** — a counting global allocator
//!   aborts on any single allocation larger than the cap, and the targets
//!   assert the returned buffer is within it. Every decode runs through a
//!   capped API (`decode_with_limit`,
//!   `decompress_to_vec{,_zlib}_with_limit`), so a stream that asks for more
//!   output fails instead of growing without bound.
//!
//! The seed corpora are the KAT vectors already in the repository (Lane 2a's
//! zlib streams, Lane 2b's PNGs/JPEGs) plus, for inflate, streams compressed
//! from deterministic inputs. Raw-DEFLATE seeds are the zlib streams' payloads
//! (header and Adler-32 stripped), so even the raw target starts from real
//! compressed data rather than random bytes.

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::RefCell;
use std::fmt;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use miniz_oxide::deflate::{compress_to_vec, compress_to_vec_zlib};
use miniz_oxide::inflate::{decompress_to_vec_with_limit, decompress_to_vec_zlib_with_limit};

/// Default decode budget for a fuzz campaign. Deliberately below the API's
/// 64 MiB default so a single successful decode cannot dominate the run; the
/// cap-enforcement path is still exercised (it is just a tighter boundary).
pub const DEFAULT_CAP: usize = 16 * 1024 * 1024;

/// Default per-target wall-clock budget, matching the dispatch's "about 60
/// seconds each".
pub const DEFAULT_SECONDS: u64 = 60;

/// Default mutation seed. A crash is reproducible from this plus the iteration.
pub const DEFAULT_SEED: u64 = 234;

/// Default bound on one mutated input, so the harness itself stays cheap.
pub const DEFAULT_MAX_INPUT: usize = 64 * 1024;

/// Hard cap on the evolving corpus, bounding harness memory.
const MAX_CORPUS_ENTRIES: usize = 256;
const MAX_CORPUS_BYTES: usize = 4 * 1024 * 1024;

/// The four fuzz targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// RFC 1950 zlib wrapper + RFC 1951 DEFLATE.
    InflateZlib,
    /// Raw RFC 1951 DEFLATE.
    InflateRaw,
    /// The `neo-image-decode` PNG path.
    Png,
    /// The `neo-image-decode` JPEG path.
    Jpeg,
}

impl Target {
    pub const ALL: [Target; 4] = [
        Target::InflateZlib,
        Target::InflateRaw,
        Target::Png,
        Target::Jpeg,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Target::InflateZlib => "inflate-zlib",
            Target::InflateRaw => "inflate-raw",
            Target::Png => "png",
            Target::Jpeg => "jpeg",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|target| target.name() == value)
    }

    /// Run one input. `Some(decoded_len)` means the decoder accepted it; `None`
    /// means it rejected it cleanly. A panic, an abort from the allocation
    /// guard, or an assertion failure is a crash.
    pub fn run(self, bytes: &[u8], cap: usize) -> Option<usize> {
        match self {
            Target::InflateZlib => match decompress_to_vec_zlib_with_limit(bytes, cap) {
                Ok(out) => {
                    assert!(
                        out.len() <= cap,
                        "zlib inflate returned {} bytes over the {cap}-byte cap",
                        out.len()
                    );
                    Some(out.len())
                }
                Err(_) => None,
            },
            Target::InflateRaw => match decompress_to_vec_with_limit(bytes, cap) {
                Ok(out) => {
                    assert!(
                        out.len() <= cap,
                        "raw inflate returned {} bytes over the {cap}-byte cap",
                        out.len()
                    );
                    Some(out.len())
                }
                Err(_) => None,
            },
            Target::Png | Target::Jpeg => match neo_image_decode::decode_with_limit(bytes, cap) {
                Ok(image) => {
                    let expected = (image.width as usize)
                        .checked_mul(image.height as usize)
                        .and_then(|pixels| pixels.checked_mul(4))
                        .expect("decoded dimensions overflow usize");
                    assert_eq!(
                        image.rgba.len(),
                        expected,
                        "{} decode returned {} RGBA bytes for {}x{}",
                        self.name(),
                        image.rgba.len(),
                        image.width,
                        image.height
                    );
                    assert!(
                        expected <= cap,
                        "{} decode returned {expected} bytes over the {cap}-byte cap",
                        self.name()
                    );
                    assert!(image.width > 0 && image.height > 0);
                    Some(expected)
                }
                Err(_) => None,
            },
        }
    }

    /// The target's seed corpus, taken from the KAT vectors already in the
    /// repository.
    pub fn seed_corpus(self) -> Vec<Vec<u8>> {
        match self {
            Target::InflateZlib => inflate_zlib_seeds(),
            Target::InflateRaw => inflate_raw_seeds(),
            Target::Png => vec![
                PNG_GRAY8.to_vec(),
                PNG_RGB8.to_vec(),
                PNG_RGB16.to_vec(),
                PNG_PALETTE8.to_vec(),
                PNG_GRAY_ALPHA8.to_vec(),
                PNG_RGBA8.to_vec(),
            ],
            Target::Jpeg => vec![
                JPEG_444.to_vec(),
                JPEG_422.to_vec(),
                JPEG_PROGRESSIVE.to_vec(),
            ],
        }
    }

    /// Format-specific byte sequences that mutation can splice in, to reach
    /// deeper parser states than a random byte flip usually does.
    pub fn dictionary(self) -> &'static [&'static [u8]] {
        match self {
            Target::InflateZlib => &ZLIB_DICTIONARY,
            Target::InflateRaw => &DEFLATE_DICTIONARY,
            Target::Png => &PNG_DICTIONARY,
            Target::Jpeg => &JPEG_DICTIONARY,
        }
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

// ── Seed corpora ────────────────────────────────────────────────────────────
//
// All of these are the exact files the Lane 2a/2b KATs use; their provenance is
// recorded in those crates' `tests/data/PROVENANCE.md`. Compile-time inclusion
// keeps the harness hermetic (no path arguments, no fetch).

static ZLIB_RATARMOUNT: &[u8] =
    include_bytes!("../../miniz-oxide-logic-tests/tests/data/zlib/ratarmount-simple.zlib");
static ZLIB_DEXADOR: &[u8] =
    include_bytes!("../../miniz-oxide-logic-tests/tests/data/zlib/dexador-test.zlib");

static PNG_GRAY8: &[u8] =
    include_bytes!("../../image-decode-logic-tests/tests/data/png/basn0g08.png");
static PNG_RGB8: &[u8] =
    include_bytes!("../../image-decode-logic-tests/tests/data/png/basn2c08.png");
static PNG_RGB16: &[u8] =
    include_bytes!("../../image-decode-logic-tests/tests/data/png/basn2c16.png");
static PNG_PALETTE8: &[u8] =
    include_bytes!("../../image-decode-logic-tests/tests/data/png/basn3p08.png");
static PNG_GRAY_ALPHA8: &[u8] =
    include_bytes!("../../image-decode-logic-tests/tests/data/png/basn4a08.png");
static PNG_RGBA8: &[u8] =
    include_bytes!("../../image-decode-logic-tests/tests/data/png/basn6a08.png");

static JPEG_444: &[u8] =
    include_bytes!("../../image-decode-logic-tests/tests/data/jpeg/tiny_non_interleaved_444.jpg");
static JPEG_422: &[u8] =
    include_bytes!("../../image-decode-logic-tests/tests/data/jpeg/non_interleaved_422_64x64.jpg");
static JPEG_PROGRESSIVE: &[u8] =
    include_bytes!("../../image-decode-logic-tests/tests/data/jpeg/progressive_restart_420.jpg");

/// PNG signatures, chunk types and a couple of chunk lengths.
static PNG_DICTIONARY: [&[u8]; 10] = [
    &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a],
    b"IHDR",
    b"PLTE",
    b"IDAT",
    b"IEND",
    b"tRNS",
    b"gAMA",
    b"sRGB",
    &[0, 0, 0, 13],
    &[0, 0, 0, 0],
];

/// JPEG markers: SOI/EOI, SOF0/SOF2, DHT, DQT, SOS, DRI, APP0/APP1, COM.
static JPEG_DICTIONARY: [&[u8]; 12] = [
    &[0xff, 0xd8],
    &[0xff, 0xd9],
    &[0xff, 0xc0],
    &[0xff, 0xc2],
    &[0xff, 0xc4],
    &[0xff, 0xdb],
    &[0xff, 0xda],
    &[0xff, 0xdd],
    &[0xff, 0xe0],
    &[0xff, 0xe1],
    &[0xff, 0xfe],
    &[0xff, 0x00],
];

/// zlib headers at the common compression levels, plus the empty stored block.
static ZLIB_DICTIONARY: [&[u8]; 8] = [
    &[0x78, 0x01],
    &[0x78, 0x5e],
    &[0x78, 0x9c],
    &[0x78, 0xda],
    &[0x08, 0x1d], // CM=1 (invalid method) / FCHECK
    &[0x78, 0xbb], // FDICT set
    &[0x03, 0x00], // empty final stored block
    &[0x00, 0x00, 0xff, 0xff],
];

/// DEFLATE block headers: stored/final, fixed-Huffman, dynamic-Huffman.
static DEFLATE_DICTIONARY: [&[u8]; 6] = [
    &[0x03, 0x00],
    &[0x01, 0x00, 0x00, 0xff, 0xff],
    &[0x00, 0x00, 0x00, 0xff, 0xff],
    &[0x4b, 0x4c, 0x4a, 0x4e, 0x49, 0x4d],
    &[0x05, 0xc0, 0x81],
    &[0xed, 0xc0, 0x01, 0x01],
];

/// Deterministic inputs to compress for the inflate seed corpora.
fn compression_inputs() -> Vec<Vec<u8>> {
    let mut xorshift = 0x9e37_79b9_7f4a_7c15u64;
    let mut random = Vec::with_capacity(16 * 1024);
    for _ in 0..16 * 1024 {
        xorshift ^= xorshift << 13;
        xorshift ^= xorshift >> 7;
        xorshift ^= xorshift << 17;
        random.push(xorshift as u8);
    }
    vec![
        Vec::new(),
        b"hello, deflate".to_vec(),
        vec![0u8; 1024],
        (0u8..=255).collect(),
        "the quick brown fox jumps over the lazy dog\n"
            .repeat(512)
            .into_bytes(),
        random,
    ]
}

fn inflate_zlib_seeds() -> Vec<Vec<u8>> {
    let mut seeds = vec![ZLIB_RATARMOUNT.to_vec(), ZLIB_DEXADOR.to_vec()];
    for input in compression_inputs() {
        seeds.push(compress_to_vec_zlib(&input, 6));
    }
    seeds
}

fn inflate_raw_seeds() -> Vec<Vec<u8>> {
    let mut seeds = Vec::new();
    for stream in [ZLIB_RATARMOUNT, ZLIB_DEXADOR] {
        if let Some(payload) = zlib_payload(stream) {
            seeds.push(payload.to_vec());
        }
    }
    for input in compression_inputs() {
        for level in [0u8, 1, 6, 10] {
            seeds.push(compress_to_vec(&input, level));
        }
    }
    seeds
}

/// Return the raw DEFLATE payload of a well-formed zlib stream (RFC 1950):
/// 2-byte header, optional 4-byte dictionary id, payload, 4-byte Adler-32.
fn zlib_payload(data: &[u8]) -> Option<&[u8]> {
    if data.len() < 6 {
        return None;
    }
    let (cmf, flg) = (data[0], data[1]);
    if (u16::from(cmf) * 256 + u16::from(flg)) % 31 != 0 {
        return None;
    }
    if cmf & 0x0f != 8 {
        return None;
    }
    let mut start = 2usize;
    if flg & 0x20 != 0 {
        start += 4;
    }
    if data.len() < start + 4 {
        return None;
    }
    Some(&data[start..data.len() - 4])
}

// ── Mutation ────────────────────────────────────────────────────────────────

/// SplitMix64: tiny, deterministic, and adequate for mutation choices.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    fn below(&mut self, bound: usize) -> usize {
        if bound == 0 {
            0
        } else {
            (self.next_u64() % bound as u64) as usize
        }
    }
}

/// Mutate `base` into `out`, bounded by `max_len`.
fn mutate(rng: &mut Rng, base: &[u8], out: &mut Vec<u8>, max_len: usize, dictionary: &[&[u8]]) {
    out.clear();
    out.extend_from_slice(base);

    let rounds = 1 + rng.below(4);
    for _ in 0..rounds {
        match rng.below(10) {
            0 => {
                if !out.is_empty() {
                    let index = rng.below(out.len());
                    out[index] ^= 1u8 << rng.below(8);
                }
            }
            1 => {
                if !out.is_empty() {
                    let index = rng.below(out.len());
                    out[index] = rng.next_u64() as u8;
                }
            }
            2 => {
                if !out.is_empty() {
                    let index = rng.below(out.len());
                    let delta = rng.below(9) as i16 - 4;
                    out[index] = (i16::from(out[index]) + delta).clamp(0, 255) as u8;
                }
            }
            3 => {
                let count = 1 + rng.below(16);
                let position = rng.below(out.len() + 1);
                for offset in 0..count {
                    if out.len() >= max_len {
                        break;
                    }
                    out.insert(position + offset, rng.next_u64() as u8);
                }
            }
            4 => {
                if !out.is_empty() {
                    let start = rng.below(out.len());
                    let count = 1 + rng.below(16);
                    let end = (start + count).min(out.len());
                    out.drain(start..end);
                }
            }
            5 => {
                if !out.is_empty() {
                    let keep = rng.below(out.len());
                    out.truncate(keep);
                }
            }
            6 => {
                if !out.is_empty() {
                    let start = rng.below(out.len());
                    let end = (start + 1 + rng.below(out.len() - start)).min(out.len());
                    let block = out[start..end].to_vec();
                    let position = rng.below(out.len() + 1);
                    if out.len() + block.len() <= max_len {
                        let tail = out.split_off(position);
                        out.extend_from_slice(&block);
                        out.extend_from_slice(&tail);
                    }
                }
            }
            7 => {
                if !dictionary.is_empty() {
                    let entry = dictionary[rng.below(dictionary.len())];
                    let position = rng.below(out.len() + 1);
                    if out.len() + entry.len() <= max_len {
                        let tail = out.split_off(position);
                        out.extend_from_slice(entry);
                        out.extend_from_slice(&tail);
                    }
                }
            }
            8 => {
                if !dictionary.is_empty() && !out.is_empty() {
                    let entry = dictionary[rng.below(dictionary.len())];
                    let position = rng.below(out.len());
                    let count = entry.len().min(out.len() - position);
                    out[position..position + count].copy_from_slice(&entry[..count]);
                }
            }
            _ => {
                if out.len() >= 2 {
                    let source = rng.below(out.len());
                    let destination = rng.below(out.len());
                    let count = 1 + rng.below(8);
                    for offset in 0..count {
                        let byte = out[(source + offset) % out.len()];
                        let target = (destination + offset) % out.len();
                        out[target] = byte;
                    }
                }
            }
        }

        if out.len() > max_len {
            out.truncate(max_len);
        }
    }
}

// ── Allocation guard ────────────────────────────────────────────────────────

/// Single-allocation cap. `0` means disarmed.
static ALLOC_LIMIT: AtomicUsize = AtomicUsize::new(0);
/// Total-live cap. Derived from the single-allocation cap when armed.
static LIVE_LIMIT: AtomicUsize = AtomicUsize::new(0);
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_BYTES: AtomicUsize = AtomicUsize::new(0);

struct CountingAlloc;

fn over_cap(size: usize, limit: usize) -> ! {
    use std::io::Write;
    let _ = writeln!(
        std::io::stderr(),
        "image-decode-fuzz: allocation of {size} bytes exceeds the {limit}-byte decode cap"
    );
    std::process::abort()
}

fn over_live_cap(live: usize, limit: usize) -> ! {
    use std::io::Write;
    let _ = writeln!(
        std::io::stderr(),
        "image-decode-fuzz: {live} live bytes exceed the {limit}-byte total guard"
    );
    std::process::abort()
}

fn note_allocation(size: usize) {
    let single = ALLOC_LIMIT.load(Ordering::Relaxed);
    if single != 0 && size > single {
        over_cap(size, single);
    }
    let live = LIVE_BYTES.fetch_add(size, Ordering::Relaxed) + size;
    PEAK_BYTES.fetch_max(live, Ordering::Relaxed);
    let total = LIVE_LIMIT.load(Ordering::Relaxed);
    if total != 0 && live > total {
        over_live_cap(live, total);
    }
}

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        note_allocation(layout.size());
        // SAFETY: forwarded unchanged to the system allocator.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note_allocation(layout.size());
        // SAFETY: forwarded unchanged to the system allocator.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::Relaxed);
        // SAFETY: `ptr` came from this allocator and `layout` is unchanged.
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let single = ALLOC_LIMIT.load(Ordering::Relaxed);
        if single != 0 && new_size > single {
            over_cap(new_size, single);
        }
        // SAFETY: `ptr`/`layout` are the caller's existing allocation.
        let out = unsafe { System.realloc(ptr, layout, new_size) };
        if !out.is_null() {
            let old = layout.size();
            let live = if new_size >= old {
                LIVE_BYTES.fetch_add(new_size - old, Ordering::Relaxed) + (new_size - old)
            } else {
                LIVE_BYTES.fetch_sub(old - new_size, Ordering::Relaxed) - (old - new_size)
            };
            PEAK_BYTES.fetch_max(live, Ordering::Relaxed);
            let total = LIVE_LIMIT.load(Ordering::Relaxed);
            if total != 0 && live > total {
                over_live_cap(live, total);
            }
        }
        out
    }
}

#[global_allocator]
static ALLOCATOR: CountingAlloc = CountingAlloc;

/// Arm the guard for a campaign. Must be called before the decode loop.
pub fn arm_allocator(cap: usize) {
    let baseline = LIVE_BYTES.load(Ordering::Relaxed);
    ALLOC_LIMIT.store(cap, Ordering::Relaxed);
    // A successful PNG decode can hold the compressed input, the decoder's
    // internal buffers (bounded by the same cap), the raw frame and the RGBA
    // frame at once. Six caps leaves headroom without allowing runaway growth.
    LIVE_LIMIT.store(
        baseline.saturating_add(cap.saturating_mul(6)),
        Ordering::Relaxed,
    );
}

pub fn reset_peak() {
    PEAK_BYTES.store(LIVE_BYTES.load(Ordering::Relaxed), Ordering::Relaxed);
}

pub fn peak_live() -> usize {
    PEAK_BYTES.load(Ordering::Relaxed)
}

// ── Crash artifact capture ──────────────────────────────────────────────────

#[derive(Clone)]
struct CurrentInput {
    target: Target,
    seed: u64,
    iteration: u64,
    bytes: Rc<Vec<u8>>,
}

thread_local! {
    static CURRENT: RefCell<Option<CurrentInput>> = const { RefCell::new(None) };
}

static CRASH_DIR: OnceLock<PathBuf> = OnceLock::new();

fn set_current(target: Target, seed: u64, iteration: u64, bytes: &[u8]) {
    let current = CurrentInput {
        target,
        seed,
        iteration,
        bytes: Rc::new(bytes.to_vec()),
    };
    CURRENT.with(|slot| *slot.borrow_mut() = Some(current));
}

/// Install a panic hook that writes the input under test before the process
/// aborts, and prints the exact replay/minimise commands.
pub fn install_crash_hook(crash_dir: PathBuf) {
    let _ = std::fs::create_dir_all(&crash_dir);
    let _ = CRASH_DIR.set(crash_dir);
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let current = CURRENT.with(|slot| slot.borrow().clone());
        if let Some(current) = current
            && let Some(dir) = CRASH_DIR.get()
        {
            let path = dir.join(format!(
                "crash-{}-seed{}-iter{}.bin",
                current.target.name(),
                current.seed,
                current.iteration
            ));
            match std::fs::write(&path, current.bytes.as_slice()) {
                Ok(()) => eprintln!(
                    "image-decode-fuzz: crash input ({} bytes) saved to {}",
                    current.bytes.len(),
                    path.display()
                ),
                Err(error) => eprintln!(
                    "image-decode-fuzz: could not save the crash input to {}: {error}",
                    path.display()
                ),
            }
            eprintln!(
                "image-decode-fuzz: reproduce with --target {} --seed {} --iterations {}",
                current.target.name(),
                current.seed,
                current.iteration
            );
            eprintln!(
                "image-decode-fuzz: minimise with --target {} --minimize {}",
                current.target.name(),
                path.display()
            );
        }
        previous(info);
    }));
}

// ── Campaign ────────────────────────────────────────────────────────────────

pub struct Campaign {
    pub target: Target,
    pub seconds: u64,
    pub iterations: u64,
    pub seed: u64,
    pub cap: usize,
    pub max_input: usize,
}

#[derive(Debug)]
pub struct Summary {
    pub target: Target,
    pub iterations: u64,
    pub accepted: u64,
    pub rejected: u64,
    pub max_output: usize,
    pub peak_live: usize,
    pub corpus_entries: usize,
    pub corpus_bytes: usize,
    pub elapsed: Duration,
}

impl fmt::Display for Summary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "target={} seconds={:.2} iterations={} accepted={} rejected={} exec_per_sec={:.0} max_output={} peak_live={} corpus={}/{}B status=ok",
            self.target.name(),
            self.elapsed.as_secs_f64(),
            self.iterations,
            self.accepted,
            self.rejected,
            self.iterations as f64 / self.elapsed.as_secs_f64().max(f64::EPSILON),
            self.max_output,
            self.peak_live,
            self.corpus_entries,
            self.corpus_bytes,
        )
    }
}

/// Run one target's campaign.
pub fn run_campaign(campaign: &Campaign) -> Summary {
    let mut corpus: Vec<Vec<u8>> = Vec::new();
    let mut corpus_bytes = 0usize;
    for seed in campaign.target.seed_corpus() {
        if seed.len() <= campaign.max_input && corpus_bytes + seed.len() <= MAX_CORPUS_BYTES {
            corpus_bytes += seed.len();
            corpus.push(seed);
        }
    }
    if corpus.is_empty() {
        corpus.push(Vec::new());
    }

    let target_index = Target::ALL
        .iter()
        .position(|candidate| *candidate == campaign.target)
        .unwrap_or(0) as u64;
    let mut rng = Rng::new(campaign.seed ^ target_index.wrapping_mul(0x9e37_79b9_7f4a_7c15));
    let mut buffer = Vec::new();

    let started = Instant::now();
    let deadline = Duration::from_secs(campaign.seconds);
    let (mut iterations, mut accepted, mut rejected, mut max_output) = (0u64, 0u64, 0u64, 0usize);

    reset_peak();
    loop {
        if campaign.iterations != 0 {
            if iterations >= campaign.iterations {
                break;
            }
        } else if iterations > 0 && started.elapsed() >= deadline {
            break;
        }

        let base = &corpus[rng.below(corpus.len())];
        mutate(
            &mut rng,
            base,
            &mut buffer,
            campaign.max_input,
            campaign.target.dictionary(),
        );
        iterations += 1;
        set_current(campaign.target, campaign.seed, iterations, &buffer);

        match campaign.target.run(&buffer, campaign.cap) {
            Some(length) => {
                accepted += 1;
                max_output = max_output.max(length);
                if iterations % 3 == 0
                    && corpus.len() < MAX_CORPUS_ENTRIES
                    && corpus_bytes + buffer.len() <= MAX_CORPUS_BYTES
                {
                    corpus_bytes += buffer.len();
                    corpus.push(buffer.clone());
                }
            }
            None => rejected += 1,
        }
    }

    Summary {
        target: campaign.target,
        iterations,
        accepted,
        rejected,
        max_output,
        peak_live: peak_live(),
        corpus_entries: corpus.len(),
        corpus_bytes,
        elapsed: started.elapsed(),
    }
}

// ── Minimiser ───────────────────────────────────────────────────────────────

/// Greedy delta-debugging reduction. `fails` must return true when `candidate`
/// still reproduces the bug. The returned vector is always a failing input.
pub fn minimize_with<F>(input: &[u8], max_evaluations: u64, mut fails: F) -> Vec<u8>
where
    F: FnMut(&[u8]) -> bool,
{
    let mut best = input.to_vec();
    let mut evaluations = 0u64;
    evaluations += 1;
    if !fails(&best) {
        return best;
    }

    // Smallest failing prefix (binary search; verified below because the
    // predicate is not guaranteed monotone).
    let mut low = 1usize;
    let mut high = best.len();
    while low < high && evaluations < max_evaluations {
        let mid = low + (high - low) / 2;
        evaluations += 1;
        if fails(&best[..mid]) {
            high = mid;
        } else {
            low = mid + 1;
        }
    }
    if low < best.len() && evaluations < max_evaluations {
        evaluations += 1;
        if fails(&best[..low]) {
            best.truncate(low);
        }
    }

    // Delete windows, halving the window size.
    let mut chunk = (best.len() / 2).max(1);
    while chunk >= 1 && evaluations < max_evaluations {
        let mut start = 0usize;
        while start < best.len() && evaluations < max_evaluations {
            let end = (start + chunk).min(best.len());
            if start == 0 && end == best.len() {
                break;
            }
            let mut candidate = Vec::with_capacity(best.len() - (end - start));
            candidate.extend_from_slice(&best[..start]);
            candidate.extend_from_slice(&best[end..]);
            evaluations += 1;
            if !candidate.is_empty() && fails(&candidate) {
                best = candidate;
            } else {
                start += chunk;
            }
        }
        chunk /= 2;
    }

    // Byte simplification: 0x00 and 0xFF are the values that most often
    // collapse a length/flag field without changing the failing path.
    for value in [0u8, 0xff] {
        let mut index = 0usize;
        while index < best.len() && evaluations < max_evaluations {
            if best[index] != value {
                let old = best[index];
                best[index] = value;
                evaluations += 1;
                if !fails(&best) {
                    best[index] = old;
                }
            }
            index += 1;
        }
    }

    best
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rng_is_deterministic() {
        let mut a = Rng::new(7);
        let mut b = Rng::new(7);
        for _ in 0..32 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn every_seed_decodes_or_inflates() {
        for target in Target::ALL {
            for seed in target.seed_corpus() {
                let outcome = target.run(&seed, DEFAULT_CAP);
                assert!(
                    outcome.is_some(),
                    "{} seed of {} bytes did not decode",
                    target.name(),
                    seed.len()
                );
            }
        }
    }

    #[test]
    fn raw_seeds_are_the_zlib_payloads() {
        for stream in [ZLIB_RATARMOUNT, ZLIB_DEXADOR] {
            let payload = zlib_payload(stream).expect("zlib stream has a payload");
            let inflated = decompress_to_vec_with_limit(payload, DEFAULT_CAP)
                .expect("the zlib payload is raw DEFLATE");
            let whole = decompress_to_vec_zlib_with_limit(stream, DEFAULT_CAP)
                .expect("the zlib stream inflates");
            assert_eq!(inflated, whole);
        }
    }

    #[test]
    fn mutation_respects_the_input_bound() {
        let mut rng = Rng::new(DEFAULT_SEED);
        let base = PNG_RGBA8.to_vec();
        let mut out = Vec::new();
        for _ in 0..2_000 {
            mutate(&mut rng, &base, &mut out, 256, &PNG_DICTIONARY);
            assert!(out.len() <= 256);
        }
    }

    #[test]
    fn minimizer_keeps_a_small_still_failing_input() {
        let mut input = vec![0u8; 64];
        input[37] = 0x42;
        let minimized = minimize_with(&input, 5_000, |candidate| candidate.contains(&0x42));
        assert!(minimized.contains(&0x42), "predicate no longer holds");
        assert!(
            minimized.len() <= 4,
            "minimizer returned {} bytes: {minimized:?}",
            minimized.len()
        );
    }

    #[test]
    fn short_campaign_runs_each_target() {
        for target in Target::ALL {
            let summary = run_campaign(&Campaign {
                target,
                seconds: 60,
                iterations: 32,
                seed: DEFAULT_SEED,
                cap: DEFAULT_CAP,
                max_input: DEFAULT_MAX_INPUT,
            });
            assert_eq!(summary.iterations, 32);
            assert_eq!(summary.accepted + summary.rejected, 32);
        }
    }
}
