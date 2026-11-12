<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: PR #100 setup S4 snapshot re-check (Dexter, relay 387 rerouted)

**Dispatch:** `Plans/DISPATCH-2026-11-10-relays-387-388-rechecks.md`. **That file is not on `origin/main` at
`59cb3c5f2`** (`git show origin/main:Plans/DISPATCH-2026-11-10-relays-387-388-rechecks.md` fails; the only record of
relay 387 is `Plans/RELAY-QUEUE.md`). This review therefore runs the Section 387 task from the queue line and from
the PM's two required corrections in PR #100 comment 6755850014 (2026-11-10 01:02), which is the operative text. The
PM should push the dispatch so 387/389/391 have their authoritative text. No RETURN for 387 or 389 exists on
`origin/main` or on any branch; `codex/pr100-recheck-janus` is absent.
**Reviewed head:** `8414cc5fe93e82e5fc55326cd194d2ce2ec4a8a0` (`claude/setup-s4-net-info`, PR #100): the fix
`2948364ea`, then a merge of `origin/main` `5ef9f7f4`.
**Previous review:** `67be4ba2` / `Plans/RETURN-2026-11-09-pr100-review-janus.md` (FAIL: F1 the torn record, item 7
the CP-13 claim; five other items passed).
**Review branch:** `dexter/pr100-recheck`, based directly on the reviewed head.
**Reviewer:** Dexter (DeepSeek family; cross-family to the Claude author).

**Verdict: PASS.** Both required corrections are implemented, and the five items Janus passed still hold. The record
is one coherent state: the generation closes the F1 tear, and the new discriminator shows the old schedule producing
a mixed record without it and a held-or-flagged one with it. Four notes below are for R3; none reopens a FAIL.

## What changed

`2948364ea` is the only change to the status path (`git diff 2948364ea 8414cc5f -- <status files>` is empty; the
merge did not touch it). It adds:
- `kernel/src/net/status_gen.rs` (37 lines): the generation, bumped by the four locked source writers;
- `if_status_logic::read_consistent` and bit 4 `STALE` (`:33`, `:36`, `:187-206`);
- the carrier-first read in `if_status::read_once` (`:36-64`);
- `dhcpv4::change` (`:203-210`) so every mutating site bumps inside `CLIENT`;
- bumps in `identity::register` / `set_link_state` (`:107`, `:148`), `ipv4::apply_lease` / `tick` (`:263`, `:299`)
  and `drivers::net::register_device` (`:63`);
- libneo `FLAG_STALE` / `stale()` (`:19`, `:54`) and the page's stale branch (`setup.rs:450`);
- the host discriminator (5 new tests, `tools/net-if-status-abi-tests/src/lib.rs:282-499`) and the THINK/SDK-ABI
  text (`docs/THINK-2026-11-02-setup-menu.md:716`, `docs/SDK-ABI.md:312`).

## Required correction 1 (F1): the record can tear — **PASS**

The protocol is a validated read over four locks plus the unlockable carrier:

- Reader: load the generation, read carriers, lease, links and IPv4, load the generation again; equal ⇒ return;
  changed ⇒ retry, at most `READ_ATTEMPTS = 4`; then the last record with `FLAG_STALE` (`if_status_logic.rs:187-206`).
- Every locked source's writer bumps **inside its own lock, after the write**: DHCP (`dhcpv4.rs:203-210`; its
  `apply_ack` also reaches `ipv4::apply_lease` while holding `CLIENT`, `:174`, preserving the existing
  `CLIENT -> INTERFACES` edge), identity registration/link change (`identity.rs:107`, `:143-149`), IPv4 add/expire
  (`ipv4.rs:249-263`, `:290-300`), device registration (`drivers/net/mod.rs:59-63`).
- Soundness: an accepted record had no generation bump between the two loads, so no *locked* source changed; the
  reader takes those writers' locks, so it cannot observe a write before its bump. The carriers are the only
  unbumped source, and they are read first, so the record equals the source states at the instant the chosen
  carrier was read — not a mix. This closes Janus's concrete `(lease, link) = (1,1) -> (0,1) -> (0,0)` schedule: the
  `drop_then_down` writer now forces a re-read (`every_interleaving_is_one_state_or_flagged`, `lib.rs:444`;
  `with_the_generation_the_record_is_one_state`, `:428`; the no-generation control at `:406`).
- The four-read bound is enforced and the never-settling case is flagged, not spun (`:481`); a quiet network is read
  once (`:496`).

## Required correction 2 (item 7): the CP-13 claim — **PASS**

The PR body's CP-13 row now names the generation protocol and the two discriminator tests, and the CP-1 row reads
"no two **network subsystem** locks" with the existing heap and `CLIENT -> INTERFACES` edges noted; the "four short
lock holds" wording is gone. The table's claims match the code except the carrier wording in N1.

## Janus's five passing items at the new head

1. **Read-only; short buffer rejected before copy — PASS.** `syscall.rs:6604-6606` returns `EINVAL` for null or
   `< 56` before `validate_user_buf` (`:6607`) and the snapshot (`:6610`); the snapshot only reads, and the new
   `bump` calls are on writer paths.
2. **Lock order / context / ISR isolation — PASS.** `read_once` holds no guard across a helper (`if_status.rs:37-64`),
   takes the device `REGISTRY`, `CLIENT`, the identity `REGISTRY` and `INTERFACES` one at a time, and `bump` is an
   atomic under the writer's lock. The existing `CLIENT -> INTERFACES` edge is unchanged (`dhcpv4.rs:174`). The
   syscall runs IF=0 (`syscall.rs:703`), takes no AI-governance lock, and never waits on a driver; `link_state()` is
   a flag or register read.
3. **Byte oracle — PASS.** The hand-laid 56-byte records are unchanged, the flag list now pins bit 4 (`lib.rs:124-142`),
   and all 15 ABI tests pass locally. See N3 for the one oracle gap.
4. **Kill-switch — PASS.** With `net-info` off, `if_status` is not compiled and 0x191 is `ENOSYS`
   (`syscall.rs:1363-1366`); `status_gen::bump()` is empty and `read()` is absent (`status_gen.rs:22-37`), so every
   writer is as it was; the page maps `ENOSYS` to "unavailable" with Continue, and `setup_menu_net_off` is unchanged.
5. **Gates — PASS.** [Run 42744006649](https://github.com/SecondStageTurbine/LogansOS/actions/runs/42744006649) is
   green on exactly `8414cc5f`: all 4 QEMU shards `success`, plus Host Tests/Format/Lint, Build Checks, Decoder
   Fuzz, the RNG-seed boot and WHPX; batch-report skipped. The PM's table (author-reported; I verified run and job
   conclusions from the API) has `setup_menu_net`, `setup_menu_net_nonic`, `setup_menu_net_off`, `setup_menu_walk`,
   `setup_menu_skip` and `setup_menu_store_try` each passing first attempt.

## Notes for R3 (not blocking)

- **N1 (medium) — the carrier is not a generation-bumping source, though the PM's bullet listed it.** `status_gen`
  covers the four locked sources; the carrier is covered by read order instead ("carriers first", THINK `:716`).
  The property holds today, but the CP-13 row's "every writer of a source bumps" is overbroad, and nothing pins the
  production read order: a later `read_once` edit that moved the carrier read after the lease would silently
  reintroduce a mix, and the model test (`lib.rs:348-356`) would not catch it because it is a separate model.
  Either bump at the drivers' carrier writers or reword to "every locked-source writer" and assert/comment the
  carrier-first order.
- **N2 (medium) — the discriminator is a model, not the production reader.** It pins `read_consistent`, `build` and
  the gap order, but not `if_status::read_once`/`choose`/the IPv4 fallback, and no test pins the production bump
  sites: a future writer that mutates `CLIENT` outside `change`, or an `INTERFACES` writer outside `apply_lease`,
  passes the suite. A `debug_assert` or a host test over the production helpers would close it.
- **N3 (small) — the oracle has no hand-laid `STALE` record.** Bit 4 is pinned as a constant (`lib.rs:124-142`) and
  decoded from a machine-built record (`:481-494`), but no literal 56-byte stale record exists; add one beside the
  leased/no-device/RTL8125 literals.
- **N4 (small) — a stale record still carries the mixed fields.** It is flagged, and the page discards it
  (`setup.rs:450`), which is the intended contract; any future consumer must check `stale()` before using the parts.

## Verification and limits

- Local at `8414cc5f`: `cargo test -p net-if-status-abi-tests` — 15 passed, 0 failed;
  `cargo test -p setup-core` — 25 passed, 0 failed. No implementation file is changed by this RETURN.
- No local QEMU (`qemu-system-x86_64` is not installed in this worker), so the gate evidence is the CI run above,
  not a local run. I read run/job conclusions from the API; the per-scenario timing lines are the PM's report.
- No close-gate/4-shard run was triggered for this documentation-only RETURN, and the reviewed head was not rebased
  or changed.

## Merge note (for the PM; not this review's verdict)

The PR's base is `5ef9f7f4`. `origin/main` is now `59cb3c5f2`, and the delta is **real code**, not docs: 57 files,
+3615/−33, including S5's keyboard work in `userland/setup-core/src/lib.rs` and `neo-init/.../setup.rs` — the same
files this PR edits. The green `8414cc5f` gate therefore does **not** transfer to a rebase: rebase onto current
`main`, re-run the gate, and re-check the setup-page composition (Network between Keyboard and Time zone) after the
conflict resolution.
