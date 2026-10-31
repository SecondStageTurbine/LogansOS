<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: Kernel CSPRNG + entropy source design

**Date:** 2026-06-17
**Author:** Prometheus
**Branch:** `prometheus/kernel-csprng-entropy-design`
**Inputs:** Dispatch `Plans/DISPATCH-2026-06-17-kernel-csprng-entropy-design-prometheus.md`; live code at `origin/main` HEAD `04d8119`; ADR-051 (WPA crypto, RustCrypto precedent); W2 branch consumer fact (`gen_snonce` = counter)
**Scope:** Design THINK only. No production code, no `ci.yml`.

---

## 1. Problem statement

LogansOS has **no kernel CSPRNG or entropy source today**. The WiFi supplicant's `gen_snonce` uses a monotonic counter (`SNONCE_CTR`), explicitly marked "NOT a CSPRNG output — adequate to bring the link up but to be replaced with a real RNG before W2 ships as a security boundary." A predictable SNonce lets an adversary forge EAPOL-Key frames and break the 4-way handshake. Every future consumer of cryptographic randomness — Arthur's cap-escalation nonces, key generation, kTLS PSK — also depends on this substrate.

This design specifies the entropy sources, DRBG construction, in-kernel API, and userland syscall for a kernel CSPRNG that is:
- **Correct:** KAT'd against published DRBG vectors (I-12), built on vetted RustCrypto primitives (Article 15.7)
- **Contained:** No AI-governance lock, no new cross-cutting invariant (Rule 2)
- **Kill-switchable:** Default-off; kernel without RNG ≡ today

---

## 2. Entropy sources on x86_64 bare metal

### 2.1 RDRAND — the primary source

**Instruction:** `RDRAND r64` — Intel/AMD on-chip hardware DRBG, outputs up to 64 bits per invocation.

**Availability detection:** CPUID.1:ECX[RDRAND] (bit 30). Present on all Intel since Sandy Bridge (2011) and AMD since Bulldozer (2011). The `x86_64` crate exposes this via `cpuid::CpuId::new().get_feature_info().map(|fi| fi.has_rdrand())`.

**Failure protocol:** RDRAND sets CF=1 on success, CF=0 on failure. The Intel SDM (Section 7.3.1.8) specifies the retry-and-fallback pattern: attempt up to 10 times; if all fail, treat the source as unavailable for this boot and fall back to secondary sources. A single failure is transient (the DRBG may be reseeding internally); persistent failure indicates a broken or emulated source.

```
// Pseudocode — not implementation
fn rdrand64() -> Option<u64> {
    for _ in 0..10 {
        let (val, cf) = unsafe { core::arch::x86_64::_rdrand64_step() };
        if cf != 0 { return Some(val) }
    }
    None  // persistent failure — source broken
}
```

**RDSEED:** CPUID.1:ECX[RDSEED] (bit 18). Available on Intel since Skylake, AMD since Zen. Returns raw entropy from the same on-die TRNG but bypasses the internal DRBG state. Useful as a secondary source to seed the kernel DRBG independently of RDRAND's internal state. **Not required for Phase 10** — RDRAND alone is sufficient if trusted.

### 2.2 The RDRAND trust question

**The controversy:** Intel SA-00581 (2018) disclosed a potential backdoor in the RNG implementation. No exploit was demonstrated; Intel released microcode updates. AMD's RDRAND has no equivalent advisory. The debate: should a security-critical OS trust the vendor's on-chip DRBG?

**Three positions:**
1. **Trust RDRAND alone:** It's NIST SP 800-90B validated on most modern CPUs. Fast, simple. Risk: single point of failure if the hardware RNG is compromised.
2. **Reject RDRAND entirely:** Use only timing-based entropy. Risk: early-boot starvation (no interrupts, no I/O yet), slow to accumulate sufficient entropy.
3. **Trust but verify — mix RDRAND with independent sources:** The conservative position. RDRAND provides the bulk of entropy; independent jitter sources provide the mixing that defeats a targeted hardware backdoor.

**Recommendation: Position 3 — mix.** RDRAND is the primary source (fast, available early, high quality), but we mix it with at least one independent entropy source before feeding the DRBG. The mixing cost is negligible (one extra SHA-256 per reseed) and it eliminates the single-source trust assumption.

### 2.3 Secondary entropy sources

**TSC jitter (early boot):** The x86 TSC is monotonic but the exact read timing varies by a few cycles due to microarchitectural effects (pipeline state, cache state). Reading `RDTSC` with variable-delay instructions between reads produces measurable jitter. **Critical constraint:** This is only useful before interrupts/I/O are available — i.e., during early boot when no other sources exist. Once the timer IRQ fires, interrupt timing is a better source.

**Interrupt timing entropy:** The exact cycle count between timer IRQs varies based on scheduling, DMA completions, and other async events. Capture `RDTSC` at each timer IRQ entry and XOR-diff with the previous value. This is the Linux `timer_interrupt()` entropy model.

**USB event timing (post-W1):** USB completion interrupts from the MT7601U or xHCI controller provide additional async timing variation. Lower priority — only available after W1 driver is running.

### 2.4 The early-boot entropy problem

At boot time, before interrupts are enabled and before any I/O drivers are running, there is **no entropy**. This is the classic Linux `getrandom(2)` blocking problem. RDRAND solves this: it's available immediately after CPUID detection (which happens during early arch init), requires no I/O, and produces high-quality output from the first call.

**Boot sequence:**
1. CPUID detects RDRAND → if present, use it for initial DRBG seed (64 bytes = 8 × RDRAND64)
2. If RDRAND unavailable → block on secondary sources (TSC jitter during early init, then timer IRQs once enabled)
3. Once timer IRQs fire → begin collecting interrupt timing entropy
4. After first reseed from mixed sources → DRBG is fully operational

**Worst case (no RDRAND):** The kernel must accumulate enough timer-jitter entropy to seed the DRBG before any consumer calls `fill_random()`. With a 1 kHz timer and ~5 bits of entropy per interrupt, this takes ~100 interrupts (~100 ms). Acceptable for boot; the `fill_random()` API blocks if the DRBG is not yet seeded (see §4.2).

---

## 3. DRBG construction

### 3.1 Choice: ChaCha20-based vs NIST SP 800-90A

**Options:**
| Construction | Standard | RustCrypto crate | Properties |
|-------------|----------|-----------------|------------|
| ChaCha20-based (Linux-style) | Linux post-2016 design | `chacha20` (already in tree via `chacha20poly1305`) | Fast, no hand-rolled crypto, 256-bit key + 128-bit counter |
| HMAC-DRBG | NIST SP 800-90A Rev. 1 | `hmac` + `sha2` (already in tree via `wifi` feature) | Standardized, KAT vectors available, slower |
| CTR-DRBG (AES) | NIST SP 800-90A Rev. 1 | `aes` (in tree via `wifi` feature) | Standardized, KAT vectors available |

**Recommendation: ChaCha20-based DRBG.** Rationale:
1. **Already in tree:** `chacha20poly1305` depends on `chacha20` — the stream cipher primitive is already a kernel dependency (kTLS). No new crate.
2. **No hand-rolled crypto:** We use `chacha20::Cipher::apply_keystream()` to generate output — the vetted RustCrypto implementation of the ChaCha20 quarter-round permutation.
3. **Performance:** ChaCha20 is significantly faster than AES-CTR or HMAC-SHA256 on x86_64 (saturates memory bandwidth vs compute-bound). For a kernel CSPRNG called per-packet in WiFi, this matters.
4. **Simplicity:** The construction is: `key = SHA256(seed_material)`, then generate output via ChaCha20 keystream with a monotonically increasing counter. Rekey on each reseed. Fewer moving parts than NIST's update/generate/rollback protocol.

**Construction (the Linux post-2016 model, adapted):**

```
State:
  key: [u8; 32]    // 256-bit ChaCha20 key
  counter: u32     // 32-bit block counter (wraps at 2^32 → force reseed)
  reseed_count: u64 // monotonic reseed counter for mixing

Seed(initial_entropy: &[u8]):
  personalization = b"LogansOS-CSPRNG-v1" || reseed_count(8 bytes LE)
  key = SHA256(initial_entropy || personalization)
  counter = 0
  reseed_count += 1

Generate(output: &mut [u8]):
  if counter > 0xF000_0000: force_reseed()  // reseed before counter wrap
  ChaCha20(key, counter).apply_keystream(output)
  counter += ceil(output.len() / 64)

Reseed(additional_entropy: &[u8]):
  personalization = b"LogansOS-CSPRNG-v1" || reseed_count(8 bytes LE)
  key = SHA256(current_key || additional_entropy || personalization)
  counter = 0
  reseed_count += 1

Force_reseed():
  collect fresh entropy from all available sources
  Reseed(fresh_entropy)
```

**Why this is not hand-rolled crypto:** The ChaCha20 quarter-rounds are in the `chacha20` crate. The SHA-256 mixing is in the `sha2` crate. We write only the **composition** — key derivation, counter management, reseed policy — which is protocol logic, not cipher implementation.

### 3.2 Reseed policy

**Interval:** Reseed every **10,000** `fill_random()` calls OR every **60 seconds** (whichever comes first). This bounds the amount of output generated from a single seed, limiting exposure if internal state is compromised. The 10,000-call interval is conservative — Linux uses 1 GB of output (~15 million ChaCha20 blocks), but our workload is much lighter.

**Entropy per reseed:** Collect at least **64 bytes** (512 bits) of fresh entropy from mixed sources:
- RDRAND: 8 × 64-bit words = 64 bytes (if available)
- Timer jitter: last 8 captured IRQ timing deltas = 64 bytes
- USB event timing: if available, append to the mix

Mix via SHA-256 as shown in the `Reseed()` construction above. The personalization string includes the reseed count to prevent replay.

**Fork/VM-reseed:** Not applicable — LogansOS has no `fork()` (capability model, single-process-per-address-space). VM migration is a future concern; if we ever support live migration, the DRBG must be excluded from the saved state and reseeded on resume (the entropy pool is inherently local to the physical host).

---

## 4. In-kernel API: `fill_random`

### 4.1 Interface

```rust
/// Fill `buf` with cryptographically secure random bytes.
///
/// Blocks if the DRBG has not yet been seeded (early boot, no RDRAND).
/// Panics if `buf` exceeds 65,536 bytes (single-call cap).
/// Never returns partial data — either the entire buffer is filled or it blocks.
pub fn fill_random(buf: &mut [u8])
```

**Placement:** `kernel/src/arch/x86_64/rng.rs` — architecture-specific (RDRAND/RDSEED are x86 instructions). The entropy collection and DRBG state live here; `fill_random` is the public leaf.

### 4.2 Lock-order posture

**Leaf primitive.** `fill_random()` takes no locks held by callers. It acquires its own internal lock (`spin::Mutex` over the DRBG state), generates output, and releases. This means it can be called from:
- Process context (normal execution)
- Interrupt context (timer IRQ handler, for entropy collection — but NOT for `fill_random()` generation itself)
- The WiFi supplicant path (W2's `gen_snonce`, which runs in process context during association)

**Critical constraint:** `fill_random()` must **not** be callable from interrupt context for generation. The DRBG generation loop (ChaCha20 keystream) is non-trivial work (~100+ cycles per 64 bytes). Doing this under an IRQ lock would block other interrupts. The entropy collection path (capturing TSC at IRQ entry) is fine in interrupt context — it's a single RDTSC + XOR into a per-CPU buffer.

**Lock order:** `rng_lock` is a leaf spinlock. It is never held while acquiring any other lock. Callers must not hold any lock when calling `fill_random()`.

### 4.3 Early-boot blocking

If the DRBG is not yet seeded (e.g., no RDRAND, still accumulating timer jitter), `fill_random()` blocks the caller until sufficient entropy is available. This is implemented as a busy-wait on an `AtomicBool` flag (`drbg_ready`) with periodic yield to allow timer IRQs to fire and contribute entropy.

**Timeout:** If the DRBG is not ready within 5 seconds of the call, panic with a diagnostic message. This indicates a broken entropy source — better to halt than to generate predictable "random" values.

### 4.4 W2 gen_snonce consumer

The W2 branch's `gen_snonce` currently uses `SNONCE_CTR` (a monotonic counter). The swap is:

```rust
// Before (W2 branch):
fn gen_snonce(&self) -> [u8; 32] {
    let mut out = [0u8; 32];
    // SNONCE_CTR — NOT a CSPRNG output
    out[..8].copy_from_slice(&(self.snonce_ctr).to_le_bytes());
    self.snonce_ctr += 1;
    out
}

// After (Phase 10 CSPRNG):
fn gen_snonce(&self) -> [u8; 32] {
    let mut out = [0u8; 32];
    fill_random(&mut out);
    out
}
```

The swap is a single-call change — no new state, no counter to manage. The `fill_random` API is designed to make this a clean drop-in replacement.

---

## 5. Userland surface: `SYS_GETRANDOM`

### 5.1 Syscall ABI

```c
/// syscall(SYS_GETRANDOM, buf, buflen, flags)
///
/// Fill `buf` with `buflen` bytes of cryptographically secure random data.
/// Returns: bytes written on success (== buflen), -errno on failure.
///
/// Flags:
///   GRND_RANDOM  (0x01) — reserved for future use (Linux compatibility)
///   GRND_NONBLOCK (0x02) — return EAGAIN if DRBG not yet seeded
///
/// Errors:
///   -EINVAL  — buflen == 0 or buflen > 65536 or unsupported flags
///   -EAGAIN  — DRBG not seeded and GRND_NONBLOCK set
///   -EFAULT  — buf points to invalid userland address
```

**Syscall number:** `SYS_GETRANDOM: u64 = 0x200` (in the 0x200 range, reserved for system services — follows the pattern: 0x000-0x0FF core, 0x100-0x17F AI, 0x180-0x19F network, 0x200+ system services).

### 5.2 Capability vs ambient — the stance

**Recommendation: Ambient (no capability required).** Randomness is an ambient primitive, like `SYS_WRITE(fd=1)` or reading the clock.

**Rationale:**
1. **Every crypto operation needs randomness.** Key generation, nonce creation, salt derivation — all require unpredictable bytes. Making randomness a capability would mean every process that does any crypto must be granted a random handle, which is boilerplate that adds no security.
2. **The threat model doesn't benefit from gating.** A compromised process already has code execution — it can generate its own predictable values if it wants to subvert crypto. The value of randomness is unpredictability to *external* observers, not access control between processes.
3. **Linux precedent:** `getrandom(2)` is ambient. Every POSIX process can call it. No capability model in Linux gates randomness because there's no security model that benefits from doing so.
4. **Article 12.1 fit:** The capability model governs *resources* (memory, devices, files, tensors) — things you can abuse to affect other tenants. Random bytes are not a resource you can weaponize against another process. They're a computation result, like the output of `SYS_WRITE` to stdout.

**Counter-argument and response:** "What about DoS — a process exhausts the entropy pool?" Answer: the DRBG is a CSPRNG. It doesn't consume entropy proportionally to output. After seeding, generating unlimited output costs CPU cycles, not entropy. There's no pool to exhaust.

---

## 6. I-12 oracle + kill-switch

### 6.1 DRBG KAT — published vectors

**The DRBG given a fixed seed is deterministic.** This makes it KAT-able: feed the same seed, get the same output. The oracle is our own construction's self-consistency against known inputs.

**Primary KAT vectors — NIST CAVP Dual-DRBG / Hash-DRBG:** While we're using ChaCha20 (not a NIST-approved DRBG), we can validate the *composition pattern* against NIST's published test infrastructure. The NIST CAVP (Cryptographic Algorithm Validation Program) publishes KAT files for DRBG constructions.

**Raw-source acquisition:** Download from the NIST CAVP site via direct shell fetch:
```bash
# NIST CTR-DRBG KAT (for reference — our construction differs, but the
# seed→generate pattern is the same composition we're validating)
curl -o drbg_kat.zip "https://csrc.nist.gov/CSRC/media/Projects/Cryptographic-Standards-and-Guidelines/documents/examples/CTR_DRBG.zip"
```

**Better approach — self-generated vectors:** Since our ChaCha20-based construction is not a NIST-standardized DRBG, there are no published KAT vectors for our exact construction. Instead:

1. **ChaCha20 stream KAT:** Use the RFC 8439 test vectors for ChaCha20 keystream generation. These are published in the RFC itself (Section 2.3.2) and can be raw-sourced from the IETF repository. Verify our `chacha20` crate produces the same keystream bytes for the same key/counter.
2. **SHA-256 seed mixing KAT:** Use NIST SHA-256 KAT vectors (`KnownAns_256.rsp` from csrc.nist.gov) to verify the seed derivation path.
3. **End-to-end DRBG self-test:** At boot, run a known-input self-test: seed with a fixed test vector, generate N bytes, compare against a precomputed golden output baked into the binary. This is the Linux `crng_init` self-test pattern.

**Golden vector generation (build-time):**
```rust
// Build-time test — seed with all-zeros key, counter=0, generate 64 bytes
// Golden output computed by the chacha20 crate with the same inputs
const SELF_TEST_GOLDEN: [u8; 64] = [
    // Computed at build time from chacha20::Cipher([0u8; 32], [0u8; 8])
    // ... (64 bytes)
];

fn drbg_self_test() {
    let mut output = [0u8; 64];
    let cipher = chacha20::Cipher::new(&[0u8; 32].into(), &[0u8; 8].into());
    cipher.apply_keystream(&mut output);
    assert_eq!(output, SELF_TEST_GOLDEN, "ChaCha20 DRBG self-test failed");
}
```

**Entropy source testing:** The entropy *source* (RDRAND, timer jitter) cannot be KAT'd — it's random by definition. What we test is the **mixing function** (SHA-256 over known inputs produces known outputs) and the **DRBG given a fixed seed** (deterministic output matches golden vector). The entropy quality is assumed from the source properties (RDRAND is NIST SP 800-90B validated on modern Intel/AMD; timer jitter provides independent mixing).

### 6.2 Kill-switch

**Cargo feature: `kernel-rng`** (default: off).

When `kernel-rng` is **off** (default):
- The `rng` module does not compile
- No new crypto deps (ChaCha20 and SHA-256 are already in tree for kTLS/wifi)
- `fill_random()` does not exist
- W2's `gen_snonce` continues using `SNONCE_CTR` (counter)
- Kernel is bit-for-bit identical to today

When `kernel-rng` is **on**:
- `rng` module compiles, DRBG initializes at boot
- `fill_random()` is available for kernel consumers
- `SYS_GETRANDOM` syscall is registered
- W2's `gen_snonce` can be swapped to use `fill_random()`

**Note on the kill-switch semantics:** A kernel without the RNG means consumers like W2-as-security-boundary simply cannot be enabled. This is correct — W2 with a counter SNonce is not a security boundary. The kill-switch gates the *feature*, not the *correctness*.

---

## 7. Threat model fit (I-14 / governance core)

**No AI-governance lock.** The RNG is a leaf primitive — it takes no locks held by the AI governance subsystem (admission, placement, eviction, tensor sharing). It does not interact with the capability table, the arena allocator, or the accel driver. It is fault-contained: a panic in `fill_random()` halts the caller, not the governance core.

**No new invariant.** The RNG does not introduce a cross-cutting invariant. It is a stateful service (DRBG key + counter) with a well-defined lifecycle (seed → generate → reseed). It does not need to be consistent with any other kernel subsystem's state. Rule 2: it is not a new invariant unless genuinely required — and it isn't.

**Entropy source fault containment:** If RDRAND fails persistently, the kernel falls back to timer-jitter entropy. If that's insufficient (e.g., single-core QEMU with no timer variation), the kernel panics at the 5-second timeout rather than producing predictable output. This is the correct failure mode — better to halt than to generate weak randomness.

---

## 8. Build slicing

### 10.1 — Entropy source + DRBG core + KAT

**Work:**
- `kernel/src/arch/x86_64/rng.rs`: RDRAND detection via CPUID, RDRAND read with retry protocol, RDSEED detection (secondary)
- TSC jitter collection during early boot (before interrupts)
- Timer IRQ entropy capture: per-CPU buffer of RDTSC deltas at timer entry
- ChaCha20-based DRBG construction: seed, generate, reseed, counter management
- SHA-256 seed mixing with personalization string + reseed count
- Boot-time self-test: known-input → golden-output assertion
- Reseed policy: interval counter (10,000 calls) + 60-second timer

**Gate:** DRBG self-test passes at boot (golden vector match). RDRAND detected and functional on target hardware. Timer jitter entropy accumulates measurable variation (>64 bits after 100 IRQs, verified by counting unique delta values).

**Kill-switchable:** `#[cfg(feature = "kernel-rng")]`. Default: off.

### 10.2 — In-kernel `fill_random` API + W2 gen_snonce swap

**Work:**
- Public `fill_random(&mut [u8])` API with early-boot blocking + 5-second panic timeout
- Lock-order verification: spinlock over DRBG state, never held across other lock acquisitions
- Call from W2 branch's `gen_snonce` (clean drop-in replacement for `SNONCE_CTR`)
- Diagnostic boot message: "RNG: seeded from RDRAND + timer jitter, self-test PASS"

**Gate:** W2 association produces a 32-byte SNonce that is unpredictable (verified by running association twice and confirming different SNonces). No lock-order violations (verified by code inspection — `fill_random` acquires only `rng_lock`, which is never nested under another lock).

**Kill-switchable:** W2's `gen_snonce` checks `cfg!(feature = "kernel-rng")` and falls back to `SNONCE_CTR` if the feature is off.

### 10.3 — Userland `SYS_GETRANDOM` syscall + libneo wrapper

**Work:**
- `SYS_GETRANDOM` (0x200) in `syscall.rs`: validate user buffer, delegate to `fill_random()`
- `GRND_NONBLOCK` flag support: return `-EAGAIN` if DRBG not seeded
- `libneo` wrapper: `pub fn getrandom(buf: &mut [u8]) -> Result<usize, Errno>`
- Integration test: userland process calls `getrandom`, verifies output varies across invocations

**Gate:** Syscall returns correct byte count. Output varies across process invocations (statistical sanity, not a formal test). `GRND_NONBLOCK` returns `-EAGAIN` before DRBG is seeded (testable by calling early in init).

**Kill-switchable:** Syscall number 0x200 is unregistered when `kernel-rng` is off. `libneo::getrandom()` returns `ENOSYS`.

---

## 9. Open decisions for PM/PL ratification

| # | Decision | Options | Recommendation | Rationale |
|---|----------|---------|----------------|-----------|
| D1 | **RDRAND trust model:** trust alone vs mix with jitter | Trust alone: simpler, faster / Mix: conservative, eliminates single-source trust | **Mix (Position 3)** | The mixing cost is negligible (one extra SHA-256 per reseed). Eliminates the "what if RDRAND is backdoored" question entirely. |
| D2 | **DRBG construction:** ChaCha20-based vs HMAC-DRBG | ChaCha20: fast, already in tree / HMAC-DRBG: NIST-standardized, published KAT vectors | **ChaCha20-based** | Already in tree via kTLS. Significantly faster (memory-bound vs compute-bound). The composition is protocol logic, not cipher implementation. |
| D3 | **Reseed interval:** 10,000 calls vs 1 GB output | 10K calls: conservative, frequent reseeding / 1 GB: Linux default, less overhead | **10,000 calls** | Our workload is light (WiFi nonces, occasional key generation). 10K calls is ~320 KB of output — reseeding at that volume costs negligible CPU and bounds exposure. |
| D4 | **Userland randomness:** ambient vs capability | Ambient: simple, matches Linux / Capability: consistent with our model | **Ambient** | Randomness is not a resource you weaponize against other tenants. Every crypto operation needs it. Gating it adds boilerplate without security benefit. |
| D5 | **RDSEED as secondary source:** include now or defer | Now: extra entropy source / Defer: RDRAND + jitter is sufficient | **Defer** | RDSEED is only available on Skylake+/Zen+. RDRAND + timer jitter mixing already satisfies the trust model. Add RDSEED later if we need more entropy diversity. |
| D6 | **Early-boot blocking behavior:** block vs panic if no entropy | Block up to 5s then panic / Block indefinitely | **Block 5s, then panic** | Indefinite blocking is a silent hang. A panic with diagnostic is debuggable and indicates a broken platform (e.g., QEMU without RDRAND passthrough). |
| D7 | **KAT vector source:** NIST CAVP vs self-generated golden | NIST: published authority / Self-generated: matches our exact construction | **Self-generated golden** (ChaCha20 RFC 8439 vectors + boot self-test) | Our construction is not a NIST-standardized DRBG. The ChaCha20 stream vectors from RFC 8439 validate the cipher; the boot self-test validates the composition. |

---

## 10. Risk register

| Risk | Impact | Mitigation |
|------|--------|------------|
| RDRAND unavailable on target (old CPU, QEMU without passthrough) | High — no entropy at boot | Timer jitter fallback + 5-second panic timeout. Document QEMU requirement: `-cpu host,rdrand,rdrand` or equivalent. |
| Timer jitter provides insufficient entropy (deterministic environment) | Medium — DRBG seed has low entropy | RDRAND is the primary source; jitter is mixing. If RDRAND works, jitter quality doesn't matter for security. |
| ChaCha20 counter wrap before reseed | Low — 2^32 blocks = 4 GB of output | Force reseed at counter > 0xF000_0000 (1 GB headroom). At 10K-call reseed interval, this is unreachable in practice. |
| Lock-order violation: caller holds another lock when calling fill_random | Medium — deadlock under contention | Code audit + static analysis: `fill_random` is documented as a leaf caller must not hold locks. Add debug assertion in test builds. |
| W2 gen_snonce swap introduces regression in association | Medium — WiFi breaks | Kill-switch: W2 falls back to SNONCE_CTR if kernel-rng is off. Test both paths. |

---

## 10. PM/PL ratification (2026-06-17)

PM verified the design (no AI-governance lock, no new invariant — Rule 2; the `fill_random` leaf-lock posture; the in-tree ChaCha20 reuse). **PL ratified D1–D7** as proposed, with two PM refinements:

| OD | Ratified | Note |
|----|----------|------|
| **D1** RDRAND trust | **Mix RDRAND + jitter** | eliminates single-source trust for one SHA-256/reseed. |
| **D2** DRBG | **ChaCha20-based** (in-tree via kTLS) | composition only, not hand-rolled crypto. |
| **D3** reseed | **10,000 calls / 60 s** | conservative; light RNG load. |
| **D4** userland | **Ambient `SYS_GETRANDOM`** | randomness is non-weaponizable; ambient like time (consistent with Article 12.1's stance — stated, not a cap). |
| **D5** RDSEED | **Defer** | RDRAND + jitter satisfies the trust model. |
| **D6** early-boot | **Block 5 s, then panic** — **REFINED** | **Fail-closed:** no-RDRAND ⇒ panic; **jitter alone is NOT trusted as a sole seed** (it is a *mixing* input only). Resolve the risk-table "jitter fallback" wording to this: we do not silently run crypto on jitter-only entropy. A no-RDRAND platform fails closed (with diagnostic), it does not degrade to weak entropy. |
| **D7** KAT source | **Self-generated golden** — **REFINED** | The *component* KATs — ChaCha20 **RFC 8439** + SHA-256 **NIST** — MUST be **raw-sourced (never WebFetch)** published anchors (the I-12 / raw-source-KAT rule). The self-generated golden is the *composition wiring* regression layered on top of those anchors — **not** the sole oracle (keeps it off pure round-trip). |

**Status:** D1–D7 ratified; the kernel-CSPRNG arc is **dispatchable** (slicing 10.1 entropy+DRBG+KAT → 10.2 `fill_random`+W2 `gen_snonce` swap → 10.3 `SYS_GETRANDOM`+libneo), each behind the `kernel-rng` feature (default-off ≡ today). Tracked as **OT-2** in `docs/OPEN-THREADS.md` (the W2-security-boundary prerequisite). Sequencing: build it when it rises in priority against the Arthur arc / the benchmark; the W2 `gen_snonce` swap (10.2) is its concrete first consumer.
