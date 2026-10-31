<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family adversarial review of Desktop DS-1b (advisory LaunchCap broker) → a Codex seat

**Type:** review only (no code to merge). **Deliverable:** a pushed RETURN file. **Why a Codex seat:** the
author and the AAG lens are Claude-family. DS-1b introduces the desktop's first *authority* object (a
`LaunchCap`) and a broker that decides what a client may spawn. It is **ADVISORY** by PM decision (the kernel's
`sys_spawn` is still ungated), but the broker's own judgement is what the later kernel `SpawnCap` rung (OT-66)
will lean on, so its logic must be right now. A same-family panel agreeing on a capability check is weak
evidence (CLAUDE.md §Cross-model diversity). The PL assigns the seat (Jarvis / Janus / Chad).

## What is under review

Branch `claude/desktop-s1-session-boot`, restarted from `main` after DS-1a #39 merged. The PM names the head SHA
when relaying. Review `git diff origin/main...origin/claude/desktop-s1-session-boot`. Design text:
`docs/THINK-2026-10-18-ds1-home-spine-shell.md` §LaunchCap.

- **Wire** (`userland/libneo/src/atrium_protocol.rs`, feature `atrium-launch`): ADR-073 request opcode **8**
  `OP_LAUNCH`, 40-byte payload = identity (24) + `module_id: u32` + reserved `u32` (must be 0) + a non-zero
  `launch_token: u64`. Module 0, a non-zero reserved field and token 0 are all rejected (`BadField`).
  Hand-laid byte oracle: `tools/atrium-logic-tests` `launch_byte_oracle`.
- **Broker** (`userland/libneo/src/atrium_launch.rs`, pure): `LaunchBroker::{mint, revoke, judge}`. Allowlists
  are enumerated module ids (a bitmask), never names. `judge` refuses with 7 (no cap for that surface, or a proof
  that does not match the token), 10 (module not on the allowlist), else returns the module name. 11 = the
  broker's own spawn failed. KAT: `launch_broker_judges_binding_token_and_allowlist`.
- **Presenting under rendezvous IPC:** the client sends `OP_LAUNCH` on its own surface channel, then the same
  token on its LaunchCap endpoint. The WM receives the token on its **own** end of the cap channel. Possession is
  proven by the channel; the token only ties the send to the request.
- **WM** (`userland/neo-init/src/desktop_session.rs`, feature `desktop-home`): `launch::mint` (a channel per
  client surface, allowlist = the probe module only, the client end granted over the session channel; the WM keeps
  its own copy until session end, because closing any handle closes that endpoint side for every holder); `launch::serve` (receive the proof → `judge` → `sys_spawn` → for the probe, `sys_wait`);
  revoke + close at session end.
- **Arthur** (`userland/neo-arthur`, feature `desktop-home`): `recv_launch_cap`; the boot-test probes are
  (a) probe granted, (b) Arthur itself refused 10, (c) a wrong proof refused 7, and (d) the **pinned advisory
  test**: a direct `sys_spawn("worker")` from Arthur, a non-broker, *succeeds* and is asserted
  (`DESKTOP launchcap advisory direct_spawn=allowed exit=1`).
- **Gate:** `desktop_home_boot` expects the mint/grant/refuse/advisory lines in order (`tools/xtask`).

## Your job: refute it

1. **The broker's judgement (the core question).** Can a client launch a module that is not on its allowlist,
   or launch while holding no cap? Try: a request naming a peer's identity on the caller's channel; a token
   replayed from an earlier request; a proof sent on the *surface* channel instead of the cap; a cap kept after
   `revoke`; a second `mint` for the same surface; module ids ≥ 32 against the `u32` bitmask; module id 0 or reserved
   ≠ 0 reaching `judge` through a decoder bypass.
2. **Possession proof under rendezvous.** Is "proven by the channel" true? The WM keeps a copy of the client end (closing it
   would cut Arthur's, see `sys_close_impl`) — can any process other than Arthur and the WM hold or obtain a copy (handle duplication, a grant forwarded by
   Arthur)? If Arthur forwards its cap to a child, what can that child launch, and is that acceptable for an
   advisory cap? What if the client never sends the proof, sends two, or closes the cap channel?
3. **Focus trap / liveness.** The WM now blocks on a second channel (the cap) after a client's request. Is that
   the same exposure as the recorded DS-1a focus-trap limitation, or is it new? Does `sys_wait` on the probe add
   a way for a client to wedge the WM (a module that never exits)? Only the probe is waited on — is the
   non-probe path correct?
4. **The pinned advisory test.** Does it really assert that a *non-broker* direct spawn succeeds, so that the
   OT-66 `SpawnCap` rung must flip it? Could it pass for the wrong reason?
5. **Byte layout.** Check the `OP_LAUNCH` request against the hand-laid oracle independently: offsets,
   endianness, the reply opcode, and the status values 7/10/11.
6. **Kill-switch.** `desktop-home` off ≡ the S3 session, bit-for-bit: is any launch code reachable without the
   feature? Does `desktop_session_boot` / `desktop_glass_boot` still assert the S3 bytes unchanged?
7. **Anything else** a Claude reviewer would structurally miss.

A finding bounces the branch only if it survives your own re-verification; uncertain items go in the RETURN as
notes. "No findings" must state what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\<your seat>     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/desktop-ds1b-crosscheck-<seat>     (git fetch origin && git checkout -B codex/desktop-ds1b-crosscheck-<seat> origin/claude/desktop-s1-session-boot)
```

```
git fetch origin
git show origin/claude/desktop-s1-session-boot:Plans/DISPATCH-2026-10-18-desktop-ds1b-codex-crosscheck.md
git diff origin/main...origin/claude/desktop-s1-session-boot
```

Optional runs: `cargo test -p atrium-logic-tests`; `cargo xtask neo-test desktop_home_boot` at `NEO_SMP=4`;
`cargo xtask run-desktop --home` to try it.

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?"
questions (faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='launch broker capability spawn allowlist'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-18-desktop-ds1b-codex-crosscheck-<seat>.md` to
`codex/desktop-ds1b-crosscheck-<seat>` and **push it**. Contents: a verdict (MERGE / MERGE-WITH-NITS / BOUNCE);
each finding with severity, file:line and a repro or reasoning; for each numbered item, what you checked.
Paste the pushed commit SHA in the reply to the PL.

**Disk reclaim:** a review lane. If you built, `cargo clean` your worktree afterwards; never delete the worktree.
