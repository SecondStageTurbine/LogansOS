<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# CLOSE-REPORT: Phase 4 BUILD-5 — DNS + Happy Eyeballs + kTLS (Phase 4 SHIP)

**Date:** 2026-05-30
**From:** PM (Cerebro)
**Disposition:** **ACCEPTED — Phase 4 ships**
**Branch:** `codex/phase-4-build-5-dns-he-ktls-jarvis`
**Sub-commits:** `aab4923` (5a), `c26f1f3` (5b), `439f853` (5c), `2c12357` (5d), `85d7a3d` (5e)
**References:**
- PM-DECISION / PL-RATIFICATION / PLAN — 2026-05-30 phase-4-build-5
- DISPATCH-2026-05-30-phase-4-build-5-dns-he-ktls-jarvis.md
- Copy Fail + CIFSwitch PM-NOTEs (kTLS posture)

---

## Disposition

BUILD-5 is **ACCEPTED on all 18 ISCs**, dispositioned in two passes: the 5a–5c
host-side crypto-boundary checkpoint (source audit) and this full close after
the 5d/5e QEMU sweep. **This close discharges Phase 4** — NeoPlutarch resolves a
hostname, races a dual-stack connection, completes a TLS 1.3 handshake, and
performs an HTTPS GET, end to end, on its own ABI.

| Sub-commit | ISCs | Result |
|---|---|---|
| 5a — DNS stub resolver + `ResolverConfig` | 5a-1…4 | PASS |
| 5b — Happy Eyeballs + RFC 6724 dest (closes FM-3-1) | 5b-1…4 | PASS |
| 5c — kTLS record layer (ChaCha20-Poly1305, sync-per-packet) | 5c-1…4 | PASS |
| 5d — `rustls` TLS 1.3 handshake + HTTP/1.1 | 5d-1…3 | PASS |
| 5e — `phase4_https_fetch` ship scenario + sweep | 5e-1…3 | PASS |

## PM Verification (independent, source audit + reported sweep)

**The cardinal rule held end to end — no hand-rolled cryptography:**

- **Kernel record AEAD (5c):** the vetted `chacha20poly1305 = "0.10"` crate
  (`default-features = false`). The only primitive-name token in `tls_record.rs`
  is the *test* `chacha20_poly1305_rfc8439_aead_vector` (RFC 8439 KAT). No
  cipher/hash/KX hand-rolled.
- **Userspace handshake (5d):** `rustls 0.23.34` (`default-features = false`,
  `custom-provider`) backed by `rustls-rustcrypto` (RustCrypto, `zeroize`). The
  provider is restricted to exactly `[TLS13_CHACHA20_POLY1305_SHA256]`
  (`tls.rs:204`) and `with_protocol_versions(&[&TLS13])` — TLS 1.3 only. No
  hand-rolled primitive in `tls.rs` (no SHA/ChaCha/AES/X25519 constants or
  functions). Entropy is a `getrandom` custom backend (rdrand) — an entropy
  source, not a crypto primitive.

**kTLS boundary invariants (5c), verified in `sys_net_tcp_enable_tls_impl`:**

- **WRITE-gated:** `tcp_socket_lookup(h, socket_rights::WRITE)` rejects a
  READ-only cap before any key handling.
- **Copy-not-share:** key material is `copy_user_bytes`-copied in; never mapped
  or shared.
- **Zeroized:** `tls_record::wipe_secret(&mut key_material)` wipes the kernel
  copy **and** `zero_user_bytes(key_ptr, …)` wipes the userspace buffer; plus
  `Drop for DirectionState` on teardown. A `key_material_is_zeroized_on_install`
  test asserts it.
- **Synchronous-per-packet:** no `async`/`spawn`/`workqueue`/callback in
  `tls_record.rs`; seal/open run inline in the net-task path — the construction
  the Copy Fail PM-NOTE required.
- **Fail-closed:** bounds checks on record length + tag; **zero**
  `unwrap`/`expect`/`panic` across `dns.rs`, `happy_eyeballs.rs`,
  `tls_record.rs`.

**Functional proof:** the `phase4_https_fetch` serial log shows the full chain —
`served tls body` → `bodytok` →
`event=PASS … https_fetch=ok dns=ok happy_eyeballs=ok tls13=rustls ktls=ok http_body=ok`.

**Sweep:** `NEO_SMP=4 NEO_TEST_ATTEMPTS=1 cargo neo-test all --no-meta` (Docker)
exit 0; `git diff --check` clean (CRLF warnings only). ff-clean from `main`
(`23f75a3` → `85d7a3d`).

## Constraints honored

- **Q1–Q3** hand-rolled RFC 1035 stub (A/AAAA), `ResolverConfig` from RDNSS
  (option-25 lift in `icmpv6.rs`) + DHCP `Lease.dns`; HEv2 + RFC 6724 prefer-IPv6
  destination — **FM-3-1 discharged**. ✓
- **Q4/Q5** hybrid kTLS, synchronous-per-packet, copy-not-share, WRITE-gated,
  vetted AEAD only. ✓
- **Q6** TLS 1.3 only, server-auth only, single suite, bundled root (DER
  fixtures under `libneo/fixtures/`). ✓
- **Q7** deterministic CI via in-harness DNS + loopback TLS server; real
  external dual-stack fetch deferred to bare-metal. ✓

## Forward-markers (carried, none blocking)

- **FM-5-1** TCP/53 truncation fallback · **FM-5-2** HEv3 protocol-priority ·
  **FM-5-3** AES-128-GCM suite · **FM-5-4** full/system trust store.
- Bare-metal deliverable: real external dual-stack HTTPS fetch + QEMU SLIRP /
  real outbound (a separate test-infra decision if pursued).

**FM-3-1 is discharged by this build.**

## Phase 4 — discharged

With BUILD-5 the Phase 4 network stack is complete: virtio-net + smoltcp phy
(BUILD-1), IPv6 + SLAAC (BUILD-2), IPv4 + DHCPv4 + dual-stack selection
(BUILD-3), hand-rolled TCP (BUILD-4), NDP (BUILD-4.5), and DNS + Happy Eyeballs +
kTLS (BUILD-5). The native sockets-equivalent ABI spans `0x180`–`0x18E`. The
ship demo — an HTTPS GET over a raced dual-stack TLS 1.3 connection — passes in
CI. **Phase 4 ships.**

## Next

Land the BUILD-5 stack to `main` in one fast-forward (`23f75a3` → tip). Phase 4
retrospective / Phase 5 opening is the next PM gate.
