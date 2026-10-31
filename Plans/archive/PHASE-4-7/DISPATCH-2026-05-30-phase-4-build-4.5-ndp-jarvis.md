# DISPATCH: Phase 4 BUILD-4.5 — IPv6 Neighbor Discovery (NDP)

**Date:** 2026-05-30
**From:** PM (Cerebro)
**To:** Jarvis
**Repo:** E:\AI Automation\AltimitNeo (GitHub: SecondStageTurbine/NeoPlutarch)
**Branch:** `codex/phase-4-build-4.5-ndp-jarvis` (create from `origin/main` **after BUILD-4 is merged**)
**Author:** SecondStageTurbine

---

## Your assignment

Implement Phase 4 BUILD-4.5: hand-rolled IPv6 Neighbor Discovery in a new
`net/ndp.rs` — the IPv6 analogue of `arp.rs` — and route IPv6 UDP/TCP wire
egress through it, replacing BUILD-4's placeholder destination MAC (FM-4-1).
smoltcp stays phy-only.

**Goal:** an IPv6 datagram to an on-link neighbor egresses with a real,
NDP-resolved destination MAC; cache miss solicits, queues, and flushes on the
advertisement.

## Before you start

1. Confirm **BUILD-4 (`fb690c9`) is merged to `origin/main`**; `git fetch
   origin main`; create the branch from the updated `origin/main`.
2. Read **PLAN-2026-05-30-phase-4-build-4.5-ndp.md** and
   **PM-DECISION-2026-05-30-phase-4-build-4.5-opening.md** (Q1–Q5).
3. Study the template + integration points: `kernel/src/net/arp.rs` (mirror it),
   `kernel/src/net/icmpv6.rs` (ICMPv6 framing + option TLV walk),
   `kernel/src/net/tcp.rs` (`ipv6_destination_mac()` placeholder — the swap
   target, ~line 1240; IPv4 already uses `arp::lookup` ~line 1150), the IPv6
   UDP egress in `socket.rs`/`ipv4.rs`, and `net::tick()` in `mod.rs`.

**Hard constraints (PM-DECISION §7):** hand-roll `ndp.rs` mirroring `arp.rs`; no
smoltcp iface; minimal neighbor cache (no full RFC 4861 NUD); leave BUILD-2 DAD
unchanged; no Router Solicitation; **no syscall/ABI change** (NDP is internal).

## Sub-commits (2 total)

### BUILD-4.5a: `net/ndp.rs` (NS/NA + neighbor cache)
- **Create:** `kernel/src/net/ndp.rs` (NEW). **Amend:** `mod.rs`.
- NS (135) / NA (136) parse+build + ICMPv6 pseudo-header checksum;
  `NeighborCache` (ipv6→MAC, TTL, lookup/insert/age_out) mirroring `ArpCache`;
  solicited-node multicast addr (ff02::1:ffXX:XXXX) + MAC (33:33:ff:XX:XX:XX);
  `ndp::inject` test hook; `ndp::tick`.
- **Closes:** ISC-4.5a-1…4.5a-4. **Commit:** `build-4.5a: ipv6 ndp neighbor solicitation/advertisement + cache (rfc 4861)`

### BUILD-4.5b: IPv6 egress integration + wire + sweep
- **Amend:** `tcp.rs` (replace `ipv6_destination_mac` placeholder with
  `ndp::lookup`/`submit_or_queue_ipv6`), the IPv6 UDP egress path, `mod.rs`
  (`net::tick` → `ndp::tick`), test harness (scenario `phase4_ndp_ipv6_neighbor`).
- Miss → NS to solicited-node multicast + queue datagram → flush on NA (mirror
  `arp::submit_or_queue_ipv4`/`flush_pending`). Loopback/self short-circuit.
- **Closes:** ISC-4.5b-1…4.5b-4. **QEMU.** **Commit:** `build-4.5b: route ipv6 udp/tcp egress through ndp + sweep + bare-metal marker`

## ISC checklist (unchecked on filing)

- [ ] ISC-4.5a-1 NS/NA build+parse round-trip + ICMPv6 checksum
- [ ] ISC-4.5a-2 injected NA populates cache; lookup returns MAC; solicited-node multicast derivation correct
- [ ] ISC-4.5a-3 cache ages out via tick
- [ ] ISC-4.5a-4 no panic / defensive parsing
- [ ] ISC-4.5b-1 IPv6 UDP/TCP egress resolves dst MAC from cache (no placeholder)
- [ ] ISC-4.5b-2 miss → NS + queue → injected NA flushes
- [ ] ISC-4.5b-3 scenario SMOKE + NEO_SMP=4; full sweep green
- [ ] ISC-4.5b-4 IPv6 SLAAC/UDP/TCP regression green; no new panic

## Key design points (PM-DECISION Q1–Q5)

- **Q1** hand-roll `ndp.rs`, mirror `arp.rs`; phy-only.
- **Q2** minimal cache (TTL aging), no full NUD.
- **Q3** BUILD-2 DAD unchanged (real NS-DAD deferred).
- **Q4** one shared `ndp` resolve/queue for both UDP and TCP IPv6 egress;
  solicited-node multicast NS on miss.
- **Q5** no Router Solicitation.

## Failure modes to avoid

1. Regressing IPv6 SLAAC/UDP/TCP egress — ISC-4.5b-4 gates it.
2. Soliciting for loopback/self — short-circuit first.
3. Parser panics — length-check before every slice (mirror `icmpv6.rs`).
4. Scope creep — no full NUD, no DAD rework, no RS, no Redirect, no syscalls.
5. ICMPv6 checksum missing the IPv6 pseudo-header.

## Testing expectations

- **4.5a (host, no QEMU):** `cargo check` clean; unit tests for NS/NA codec +
  checksum, cache lookup/insert/age, solicited-node multicast derivation
  (via `ndp::inject` + synthetic tick).
- **4.5b (QEMU):** `phase4_ndp_ipv6_neighbor` SMOKE + NEO_SMP=4; full sweep.

## Return artifact

File `Plans/RETURN-2026-05-30-phase-4-build-4.5-jarvis.md`: sub-commit hashes +
messages; ISC results; exact cargo/test/Docker invocations + output; code-review
notes; blocker links.

## Dispatch assumptions

BUILD-4 merged to main; `arp.rs` template + `icmpv6.rs` framing on main; IPv6
egress placeholder (`tcp.rs ipv6_destination_mac`) present.

## Escalation path

`Plans/BLOCKER-2026-MM-DD-<description>.md`; stop on the affected sub-commit.

---

**After BUILD-4 lands on main: fetch, create the branch, read the PLAN, begin
BUILD-4.5a. 4.5a is host-verifiable; pause for the harness at 4.5b.**
