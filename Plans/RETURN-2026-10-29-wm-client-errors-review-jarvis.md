<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: WM client errors, relay #142 / #189

Reviewer: Jarvis. Worktree: `E:\AI Automation\workers\jarvis`.
Branch: `codex/review-bundle-2026-10-29b-jarvis`. **PASS.**
Read squash `46d6ba1e` and the complete PR arm table. Unless identified as
the squash, line references use main `87458b7e`.

1. **PASS.** `userland/neo-init/src/desktop_session.rs:1501` handles refused
   Commit/CommitDamage, :1584 Configure, the Destroy and Yield branches
   following them refuse registry errors and continue, and the catch-all
   sends BadState. `Fault::Client` never takes the present/placement failure
   path. `apply_configure` (:603, :610, :613) checks unpublished, wrong identity,
   and wrong dimensions/serial before clearing pending state or moving the
   frame. `commit` (:1879) asks the registry before copying/presenting pixels.
   The table includes all these branches and undecodable messages. Remaining
   Wm(92) paths follow an accepted identity and the WM's own published size;
   they are placement/registry-consistency failures, not unchecked client
   fields. Present failures, failed reply sends and closed channels stay fatal
   as explicitly scoped. The handshake and Lens loop are outside this change.
2. **PASS.** `userland/libneo/src/atrium_protocol.rs:398` requires the whole
   header, correct magic/version and a request opcode (reply bit clear).
   Unknown request opcodes still identify a request for a refusal.
   `Incoming::Undecodable` (`desktop_session.rs:1477`) replies only with a
   target, otherwise logs/drops; the loop then continues. Headerless nonempty
   bytes therefore do not terminate the session. A zero-byte SYS_SEND is a
   kernel no-op, not a delivered empty protocol message. The hand-laid
   `request_target_byte_oracle` (`tools/atrium-logic-tests/src/lib.rs:1192`)
   checks truncated payload, 23-byte header, bad magic/version, unknown op,
   reply bit and reply bytes including status offset 24.
3. **PASS — source discrimination and author negative control.**
   `userland/neo-arthur/src/showcase_surface.rs:647` at `130a7ef3` sends stale
   commit, outside damage, wrong-channel destroy/yield, unexpected focus,
   unpublished configure, truncated and headerless messages; :541 sends a
   wrong-serial configure before the valid ack. xtask's ordered sequences
   (`tools/xtask/src/main.rs:9469` at that squash) require refusal statuses,
   the drop, and later successful configure/resize. Old code fails on the
   first stale commit with neo-init exit 25; #189 records all three negative
   runs. This review traced that old branch, but did not rerun those VMs.
4. **PASS as inspected author binary evidence, not a new measurement.**
   #189 records `.text` growth of 2144–2864 bytes across five WM feature sets,
   consistent with the added refusal envelope/error split. Product Arthur's
   probe call/code is cfg `arthur-refusal-probe-test`; it is absent in product.
   The reported equal 304036-byte text and normalized disassembly comparison
   establish same function instructions with changed order. No claim of
   byte-identical ordered Arthur `.text` is made here.

Executed `cargo test -p atrium-logic-tests request_target --locked --quiet`
at `130a7ef3`: 1 passed. Independently queried
[batch 14b](https://github.com/SecondStageTurbine/LogansOS/actions/runs/40817887458):
success at `67f023bc99720804dc1e3a266cd50910c8325299`. RAG healthy; no new
QEMU or binary-pair build claimed. Synchronous reply parking remains the
separate CP-57/L2b concern; this RETURN checks status handling on completed
protocol exchanges.
