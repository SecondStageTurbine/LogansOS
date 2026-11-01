<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 233 — PR #29 fetch-broker review — Jarvis

Reviewed head: `79bc78171eb68d96c40d8caf636bf98c42de7182` (fetched `pull/29/head`).
RETURN branch: `codex/pr29-review-jarvis`, based on `origin/main` at
`10e10ee99b282ae163749748a9e4427ef5132deb`. All source references below are at the reviewed PR head.
Overall: **FAIL** against Section 233's requirements.

1. **FAIL — the submitted content-process scenario retains its own root dial capability.**
   `kernel/src/main.rs:443` installs the root NetDial at neo-init handle 0. The new scenario explicitly uses
   neo-init as content (`userland/neo-init/src/main.rs:8458`), then passes that handle to the worker
   (`userland/neo-init/src/main.rs:8522`) and broker (`userland/neo-init/src/main.rs:8537`). Spawn clones the
   capability; it does not remove the caller's handle (`kernel/src/arch/x86_64/syscall.rs:1931`). There is no
   subsequent removal of content's root and no direct-dial refusal assertion. Concrete input: immediately after
   broker spawn, content calls `sys_net_dial(NetDialCap(Handle(0)), "phase4.test", 4433)`. Its wildcard root
   passes the authority/scope checks and retains NON_PUBLIC, so it can reach the fixture directly. This finding
   concerns the PR's new content scenario; the PR does not add a production browser-launch integration.

   Egress-path inventory with `net-dial-cap` enabled: `SYS_NET_DIAL` checks a handle in the current process,
   USE + DIAL, then host/port scope (`kernel/src/arch/x86_64/syscall.rs:5393`, `:5400`, `:5415`); this stops a
   genuinely capless browser, but not the content above. Legacy `SYS_SOCKET_CREATE`, `SYS_SOCKET_BIND`,
   `SYS_SOCKET_SENDTO`, `SYS_NET_CONNECT_TCP`, and DNS egress via `SYS_NET_RESOLVE` return ENOSYS
   (`kernel/src/arch/x86_64/syscall.rs:1246`). `SYS_SOCKET_SEND` accepts an existing TCP socket with USE + WRITE
   (`kernel/src/arch/x86_64/syscall.rs:5630`, `:5076`) and refuses the datagram fallback (`:5651`); it cannot
   create a connection. `SYS_NET_LISTEN_TCP` requires a wildcard root NetDial with USE + DIAL (`:5170`), and
   `SYS_NET_ACCEPT_TCP` requires an existing listener with USE + READ (`:5223`, `:5055`). Inheritance and grant
   can supply an existing socket or dial capability if a launcher/donor permits it (`:1931`, `:7955`); the broker
   request loop itself returns encoded bytes, not granted handles (`userland/fetch-broker/src/main.rs:72`, `:85`).
   Scope derivation requires an existing parent with DERIVE (`kernel/src/sched/process.rs:3417`, `:3427`);
   generic derivation cannot amplify rights (`:3380`). Launch real content separately with only its channel,
   and assert its direct dial is EACCES.

2. **FAIL — address filtering covers every candidate, but does not refuse every required address class.**
   `kernel/src/net/happy_eyeballs.rs:108` resolves once, filters the entire candidate vector at `:112`, and
   passes only that vector to the attempt loop (`:121`, `:131`, `:73`). Thus a later Happy Eyeballs attempt
   cannot recover an already-filtered candidate. Loopback, RFC 1918, link-local, multicast, private mapped IPv4,
   and the well-known NAT64 /96 are refused by `kernel/src/sched/net_dial_cap_logic.rs:49`, `:79`, `:86`, `:98`.
   However, `:79` accepts IPv4-mapped IPv6 when the embedded IPv4 is public: concrete input
   `https://[::ffff:8.8.8.8]/` passes both URL policy and `v6_public`, contrary to the dispatch's blanket
   mapped-address refusal. The NAT64 test at `:86` recognizes only `64:ff9b::/96`; concrete input
   `https://[64:ff9b:1::7f00:1]/` in the local-use NAT64 prefix also passes both checks. A DNS AAAA candidate
   containing either address survives the same kernel filter on any attempt. Host probes confirmed acceptance;
   they do not claim a successful routed connection. The `fixture-loopback` build also deliberately preserves
   NON_PUBLIC for `phase4.test` (`userland/fetch-broker/src/main.rs:129`, `:354`); that documented test exception
   is separate from these classifier gaps.

3. **FAIL — hand-laid byte oracles pass, but channel truncation can hide malformed oversized requests.**
   The independent FETCH, HEAD, BODY, END, ERROR and CANCEL fixtures are present at
   `userland/fetch-proto/src/wire.rs:348`, `:373`, `:394`. Checked slicing, field limits, UTF-8 checks and trailing
   byte rejection are enforced at `:227`, `:248`, `:259`, `:309`, `:329`; malformed bytes supplied in full are
   rejected. The broker receives into 65,536 bytes and decodes the returned prefix without treating a full buffer
   as truncation (`userland/fetch-broker/src/main.rs:51`, `:66`, `:72`). IPC copies and returns
   `min(sender_len, max_len)` in both rendezvous orders (`kernel/src/ipc/mod.rs:170`, `:243`).
   Concrete input: encode FETCH id 1, GET, URL `https://example.com`, sixteen headers all named `X`, the first
   fifteen values consisting of 4,096 ASCII `a` bytes each and the final value of 3,987 `a` bytes. This is exactly
   65,536 bytes within the individual field limits. Append `0xff` and send all 65,537 bytes in one channel send.
   Full-message decode returns Trailing; SYS_RECV discards the last byte, and the broker decodes and serves the
   valid prefix. A host probe reproduced these exact lengths and decode results. Add an aggregate wire limit and
   receive enough to detect overflow, or expose message truncation as an error/original length.

4. **FAIL — the final narrowed leaf is nontransferable, but root/scoped dial rights can transfer and survive their donor.**
   The broker's final leaf has only USE + DIAL (`userland/fetch-broker/src/main.rs:133`), so spawn/grant reject
   that leaf for missing GRANT. Normal connection cleanup closes its socket and revokes the scope and descendants
   (`:178`); process teardown drains locally held handles (`kernel/src/sched/process.rs:3166`, `:4687`).
   However, root rights include GRANT (`kernel/src/sched/process.rs:258`, `:389`), and scope derivation preserves
   those rights (`:3434`). Neither spawn's generic GRANT arm (`kernel/src/arch/x86_64/syscall.rs:1931`) nor
   channel grant (`:7955`; `kernel/src/sched/process.rs:3873`, `:3900`) prohibits NetDial transfer. Concrete
   sequence: derive scope `phase4.test:4433` from a root, spawn a child with that scope (or grant it with USE +
   DIAL to a waiting receiver), then let the donor exit normally. Teardown retires/reparents descendant edges
   rather than revoking the recipient's capability (`kernel/src/sched/process.rs:3123`, `:3173`), so the child
   can still dial with it. This is inherited capability-delegation behavior, not a newly introduced graph bug,
   but it does not satisfy the dispatch's process-bound, nontransferable dial-right requirement. Distinguish
   launcher authority from process-bound request authority if that stronger lifetime is required.

5. **FAIL — feature-off behavior is changed outside the new feature gates.**
   Existing root holders retain the original dial path when the new feature is off
   (`kernel/src/arch/x86_64/syscall.rs:5427`), and receive NON_PUBLIC when it is on
   (`kernel/src/sched/process.rs:389`). TCP connection state transitions are otherwise untouched by this diff.
   Nevertheless, `kernel/src/net/tcp.rs:1093` unconditionally adds serial output when loopback injection fails.
   Concrete input: queue a loopback segment with a truncated TCP header, then call `pump_loopback_egress()`;
   injection fails at `kernel/src/net/tcp.rs:781` and now prints where the previous build silently discarded it,
   even with both new features disabled. Existing libneo TLS callers also unconditionally change:
   `userland/libneo/src/tls.rs:40`, `:528` increase empty receive polls from 512 to 65,536. Concrete input: a peer
   producing 512 EAGAIN results and data on poll 513 formerly returned Limit and now succeeds. The server also
   changes handling of application data accompanying Finished (`:329`). These can be useful fixes, but need
   separate changes or gating to meet the stated previous-build kill-switch. Binary equality was not measured.

Validation: `cargo test -p fetch-proto` — 28 passed; `cargo test -p cap-graph-logic-tests` — 9 passed;
five temporary standalone host probes against the reviewed fetch-proto library — 5 passed, covering the two
address acceptance counterexamples, standard denied classes, oversized field rejection, and the exact channel-prefix
counterexample above. Probes were kept outside the repository. Capability transfer/lifetime and feature-off findings
are source traces, not boot-test claims. QEMU and xorriso are unavailable here; boot scenarios were not rerun.
