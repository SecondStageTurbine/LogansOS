<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: the browser's fetch broker (OT-61 ladder rung 5)

**Seat:** D. **Date:** 2026-10-30. **Status:** design for review, no code yet. **Ladder:**
`Plans/DISPATCH-2026-10-30-seat-d-browser.md` rung 5 ("the browser gets no dial capability of its own; fetch goes
through a broker that holds the net cap; composes with lane L1's `net-dial-cap` in image A; HTTPS uses the existing TLS
path"). **Depends on nothing above it in the ladder.** It is first-party code over primitives that are on `main`
today, so it can be built while C2.4 (Stylo) waits on J.c.'s Python question (#249).

## 1. What exists

- **The dial cap** (`net-dial-cap`, `kernel/src/arch/x86_64/syscall.rs`): with the feature on, every legacy egress
  root (`SYS_SOCKET_CREATE`/`BIND`/`SENDTO`, `CONNECT_TCP`) is closed. `SYS_NET_DIAL(name, port, cap)` is the only way
  out. libneo has `sys_net_dial`, `sys_net_dial_cap_derive(parent, host, port)` (a child cap scoped to one host and
  port) and `sys_revoke`. `phase4_https_fetch` proves all of it: no cap is `EACCES`, an in-scope dial works, an
  out-of-scope port is `EACCES`, and a revoked cap is `EACCES`.
- **TLS** (`libneo::tls`): rustls 0.23 with the `rustls-rustcrypto` provider, unbuffered API, `https_get(socket,
  host, path, roots, body_out)`.
- **Content confinement** (C2.1/C2.3a): a browser-runtime process holds no dial cap, and std's `TcpStream::connect`
  refuses (`servo_host_smoke`'s third refusal).
- **Lane L1.2** (seat A, queued): `net-dial-cap` and `interface-caps` in image A. "Only the network broker or the WM
  holds" the root dial cap. **This rung is that broker, for the browser.** It doesn't wait for L1.2: the scenario runs
  on a `net-dial-cap` test kernel, like `phase4_https_fetch` does today.

## 2. The design

```
 content process (browser-runtime cap, NO dial cap)
      │  channel: FETCH request ─────────────▶  fetch broker (holds the root NetDialCap)
      │                                           1. parse + check the URL (policy, §3)
      │                                           2. derive a cap for exactly host:port
      │                                           3. dial, TLS (verify against the root store), HTTP/1.1
      │  ◀──────────── HEAD frame, BODY frames    4. stream the response back
      │  ◀──────────── END / ERROR frame          5. revoke the derived cap; close
```

- **One broker per browser instance**, spawned by whoever launches the browser (the WM in image A). It is handed the
  root `NetDialCap` and one channel per content process. The content process is handed only its channel. The broker
  is the only process in the browser bundle that can reach the network. A compromised content process can ask for
  URLs, and that's all it can do.
- **A derived cap per request, revoked at the end.** The broker never dials with its root cap directly. It derives
  `host:port`, dials with the child, and revokes the child when the response ends, so even a bug in the broker's HTTP
  code can't reuse a connection's authority for another host. That's the same pattern `phase4_https_fetch` already
  proves.
- **No new syscall and no kernel change.** The A4 manifest is untouched.

### 2a. A dial right for non-public addresses (kernel, sub-rung 5b)

A new semantic right on `Handle::NetDial`, `net_dial_rights::NON_PUBLIC`. Root caps carry it, so every existing
scenario and every existing holder dials exactly as today: that's the kill-switch, bit-identical for current caps.
`SYS_NET_DIAL_CAP_DERIVE` takes a rights mask (a fifth argument; derivation only ever narrows), and the broker
derives its per-request caps **without** `NON_PUBLIC`. In `happy_eyeballs::dial_name`, a cap without the right drops
every candidate address that `fetch_proto::policy::ip_allowed` would refuse, after the kernel's own resolution and
before connecting. If none is left, the dial is `EACCES`. Enforcement happens at the one point where the address is
final, so there's no time-of-check gap. That needs the CP bank consult (the net-lock IF-off rule, CP-37) and a
cross-family review, like any cap-path change.

### The wire format (needs a byte oracle, per the standing rule)

Little-endian, one message per channel `send`:

| Message | Direction | Layout |
|---|---|---|
| `FETCH` | content → broker | `u8 tag=1`, `u32 request_id`, `u8 method` (1 = GET, 2 = HEAD; POST later), `u16 url_len`, `url` (UTF-8, ≤ 2048), `u16 header_count`, then per header `u16 name_len, name, u16 value_len, value` |
| `HEAD` | broker → content | `u8 tag=2`, `u32 request_id`, `u16 status`, `u32 content_length` (`u32::MAX` = unknown), `u16 header_count`, headers as above, `u16 final_url_len`, `final_url` (after redirects) |
| `BODY` | broker → content | `u8 tag=3`, `u32 request_id`, `u16 len`, `len` bytes (≤ the channel's message size) |
| `END` | broker → content | `u8 tag=4`, `u32 request_id` |
| `ERROR` | broker → content | `u8 tag=5`, `u32 request_id`, `u8 code` (policy refused / DNS / connect / TLS (with the `TlsError::code` byte) / HTTP / too large / timeout) |
| `CANCEL` | content → broker | `u8 tag=6`, `u32 request_id` |

The build ships a hand-laid-out byte example of each message in the test suite, asserted against independently of
the encoder and decoder (ADR-028's rule).

## 3. Policy (what the broker refuses)

- **Schemes:** `https` only at first. `http` is a J.c. decision (§6). `file:`, `data:`, `blob:` and `about:` never
  reach the broker: the content process handles them itself, or they're refused.
- **No private destinations (SSRF):** refuse IP literals in loopback, RFC 1918, link-local, CGNAT, ULA and multicast
  ranges, and refuse `localhost` and `.local` (`fetch_proto::policy`, built). A **name that resolves** to one of those
  must be refused too, and the broker can't do that check itself: with `net-dial-cap` on, `SYS_NET_RESOLVE` is closed
  (DNS is egress), and there's no peer-address query. Resolving and then dialing by name would leave a rebinding window
  anyway. **So the check goes into the kernel's dial, as a cap right (§2a).**
- **Ports:** 443 (and 80 if `http` is allowed). The fetch standard's "bad ports" list is refused outright.
- **Limits per content process:** a fixed number of requests in flight, a response size ceiling, and an idle
  timeout. A2 (the per-content memory quota) covers the content side; these cover the broker, which serves every tab.
- **Redirects:** at most 20 (the fetch standard's limit). Every hop is re-checked against the whole policy, and an
  https → http hop is refused.

## 4. What `libneo::tls` lacks for the real web, and how to fill it

| Gap | Today | Needed | How |
|---|---|---|---|
| TLS 1.2, suites | 1.3, **one suite** (`TLS13_CHACHA20_POLY1305_SHA256`): after the handshake, the kernel's record layer (`sys_net_tcp_enable_tls`) carries the traffic, and it implements only that suite | Many servers are 1.2-only or don't offer ChaCha20 | **The broker keeps the record layer in user space**: rustls' unbuffered API (`ReadTraffic`/`WriteTraffic`) instead of `dangerous_into_kernel_connection`, so the suites are whatever the provider offers (AES-GCM and ChaCha20, 1.2 and 1.3). No kernel change; the kernel offload stays as it is for its existing users. |
| Time | `FixedTime` (2026-05-30) | Certificate validity against real time | `CLOCK_REALTIME` on an `rtc-epoch` kernel (OT-85). The broker refuses to start on a kernel without it, rather than trusting a fixed date. |
| Roots | Caller-supplied DER | The public CA set | **Mozilla's `certdata.txt` (MPL-2.0)**, fetched raw with `git`/`curl` (the byte-oracle rule), converted to DER by a small xtask tool, and pinned with its source commit. Not `webpki-roots`, which is CDLA-Permissive-2.0, a licence the standing list doesn't name (§6). |
| HTTP | GET, whole body into one buffer, `Connection: close` | Status, headers, chunked transfer, content-length, streaming, redirects | `httparse` (MIT/Apache, last release 2025-03-03) for the head; our own chunked decoder (small, with its own byte oracle). |
| Content-Encoding | none | gzip/deflate (nearly every site) | `flate2` + `miniz_oxide`, both already in the tree. brotli (`brotli-decompressor`, BSD-3/MIT) later. |
| Crypto provider | `rustls-rustcrypto` 0.0.2-alpha | Same | **Its last release was 2024-04-24 (about 2.5 years ago: abandoned by the standing rule).** It's already in the tree for `phase4_https_fetch`. Seat D maintains our copy for the broker. Replacing it is a separate decision (the obvious candidates are `aws-lc-rs` and `ring`, both C/asm, so neither is a drop-in for us). |

## 5. Prior art (patterns only, no code borrowed)

- **Chromium's Network Service:** one privileged process does all fetches for sandboxed renderers through a narrow
  IPC interface (`URLLoaderFactory`). This design has the same shape, minus cookies and cache at first.
- **Fuchsia's `fuchsia.net.http.Loader`:** a capability-routed HTTP loader component; a component without the
  protocol in its namespace can't fetch. That's the closest analogue to our cap model.
- **WebKit's NetworkProcess** and **Firefox's socket process:** the same split, and the same lesson. **Cookies and the
  HTTP cache belong in the broker**, keyed by top-level site, not in content. That's out of scope for this rung, and
  the protocol leaves room for it (headers in both directions).

## 6. Decisions for J.c. (none blocks the first build)

1. **`http://` (cleartext):** allow, refuse, or allow behind a per-site prompt? Recommendation: refuse in the first
   build (https only), then decide before rung 7's real-site battery.
2. **The CA store:** Mozilla `certdata.txt` (MPL-2.0, converted in-tree) is recommended over `webpki-roots`
   (CDLA-Permissive-2.0). It needs no new licence on the list.
3. **`rustls-rustcrypto` going unmaintained:** keep maintaining our copy for now (recommended), or choose a
   replacement provider before the browser ships.

## 7. The build, in three parts

- **5a, `fetch-proto` (built):** the wire format, the URL policy (including the Fetch standard's 83 bad ports, raw
  from whatwg/fetch @ `59afe349`) and the chunked decoder, all host-tested with byte oracles.
- **5b, the `NON_PUBLIC` dial right (§2a):** kernel. Scenario: a derived cap without the right can't reach the loopback
  fixture by name, and the root cap still can.
- **5c, the broker** and the scenario below.

## 7.1 The broker's gate

- `userland/fetch-broker` (no_std libneo, like the TLS fixtures; the broker needs no `std`). A content-side
  `fetch-client` module in the std smoke crates.
- **Scenario `fetch_broker`** on a `net-dial-cap` kernel, using the existing loopback HTTPS fixture server (the same
  hermetic fixture as `phase4_https_fetch`, extended to serve a redirect, a chunked body and a gzip body). The content
  process proves all of these:
  - it fetches through the broker (body bytes match);
  - its own `SYS_NET_DIAL` is `EACCES`;
  - a private-address URL is refused by policy;
  - a redirect is followed and re-checked;
  - a response over the ceiling is cut off with `ERROR(too large)`;
  - after the response, the derived cap is revoked (the harness counts the broker's caps).
- Plus the byte-oracle KATs for the wire format and the chunked decoder, as host tests.
- Kill-switch: an opt-in crate and scenario. The default and product images are unchanged until lane L1.2 and rung 7
  wire it into image A.
