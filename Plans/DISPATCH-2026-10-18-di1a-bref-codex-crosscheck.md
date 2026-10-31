<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family adversarial review of DI-1a (B-ref) → a Codex seat

**Type:** review only (no code to merge). **Deliverable:** a pushed RETURN file.

**Why a Codex seat.** DI-1a changes the cross-CPU input path (the delivery lock now also guards an
outstanding-record ring), adds an IPC primitive (`SYS_TRY_RECV`), and adds an authority capability. The
author and the AAG panel are Claude-family; CLAUDE.md requires a different-family refute-lens for the
concurrency core. Jarvis wrote the review that chose B-ref, so a different seat is preferable for
independence, but the PL assigns it (Jarvis / Janus / Chad).

## What is under review

PR #46, branch `claude/desktop-s2-fonts`. It is stacked on #44 (`claude/s1-f1-hid-contention`): review
`git diff origin/claude/s1-f1-hid-contention...origin/claude/desktop-s2-fonts` for DI-1a alone. The design
is THINK §8 (#45, `origin/claude/di1-think-bref`).

- **Pure:** `kernel/src/atrium/pointer_route_logic.rs`, a 64-slot record ring tagged `seq & 0xffff`.
  Records are consumed exactly once; an overwrite makes a record stale; a lost transition forces `RESYNC`;
  sequence exhaustion fails closed.
- **Authority:** `Handle::InputSystem` (`kernel/src/sched/process.rs`). Kernel-minted into neo-init only
  (`kernel/src/main.rs`), rights `USE|SYSTEM` with no DERIVE/GRANT. `holds_input_system()` is exact.
- **Routing:** `kernel/src/atrium/focus.rs` (`Delivery { routes, ring }`, `claim_pointer_route`,
  `mint_records`, `forward_record`, `Drop`).
- **Syscalls:** `kernel/src/arch/x86_64/syscall.rs` 0x1B1 CLAIM, 0x1B2 FORWARD, 0x1B3 SAK_TAKE,
  0x1B4 TRY_RECV. `Channel::try_recv` is in `kernel/src/ipc/mod.rs`.
- **WM:** `userland/neo-init/src/desktop_session.rs` `mod pointer`, the polling loop, the sprite.
  Arthur: `EPERM` = not focused.
- **Discriminators:** QEMU `di1_forward`, `desktop_pointer_boot`; host `pointer_route_kats`, `sak_kats`.

## Your job: refute it

1. **Exactly-once.** Can a record be forwarded twice, forwarded after a drop, forwarded after its slot was
   reused, or forwarded by a stale route after a reclaim? Can claim/reclaim race a mint (`DELIVERY` is held
   by both)?
2. **Authority.** Can any process other than the session root obtain or exercise `InputSystem` (derive,
   grant, inherit on spawn, handle copy)? Does anything still consult `scanout_authorized` for input?
3. **Forge and misroute.** The buttons and phase a client receives must come from the record. Can the WM
   choose them? Can it deliver to a focus it holds no ACTIVATE handle on? Is delivering to a non-active
   focus harmful?
4. **`SYS_TRY_RECV`.** Is it exactly `recv`'s send-blocked handoff under one guard? Can it park, lose a
   sender, double-wake, or leave the endpoint inconsistent? CP-14/15/16.
5. **Locks / IF.** The forward path resolves handles first, then takes `DELIVERY` with no handle-table or USB
   lock held. Check every `DELIVERY` acquisition's IF state (CP-37).
6. **Bounds.** Mint and forward are O(1) plus one bounded enqueue. Confirm there is no uncapped lookup
   (CP-54) and no allocation.
7. **Kill-switches.** Kernel `atrium-pointer-forward` off must be ADR-074 bit-for-bit; neo-init
   `desktop-pointer` off must be DS-1 exactly.
8. **Discriminator validity.** Can `di1_forward` or `desktop_pointer_boot` pass vacuously? What is untested
   (overflow and RESYNC only host-tested; userland reclaim; the authority negative only with enforcement
   on)?
9. **Anything else** a Claude reviewer would structurally miss.

A finding counts only if it survives your own re-verification. Uncertain items go in the RETURN as notes.
"No findings" must state what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\<your seat>     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/di1a-bref-crosscheck-<seat>         (git fetch origin && git checkout -B codex/di1a-bref-crosscheck-<seat> origin/claude/desktop-s2-fonts)
```

```
git fetch origin
git show origin/claude/desktop-s2-fonts:Plans/DISPATCH-2026-10-18-di1a-bref-codex-crosscheck.md
git diff origin/claude/s1-f1-hid-contention...origin/claude/desktop-s2-fonts
git show origin/claude/di1-think-bref:docs/THINK-2026-10-18-desktop-di1-pointer-decision.md
```

Building and running are optional: `cargo test -p atrium-logic-tests`;
`cargo xtask neo-test di1_forward` and `cargo xtask neo-test desktop_pointer_boot` at `NEO_SMP=4`.

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?"
questions (faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='pointer record ring consume once forward'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-18-di1a-bref-codex-crosscheck-<seat>.md` to
`codex/di1a-bref-crosscheck-<seat>` and **push it**. Contents: a verdict (MERGE / MERGE-WITH-NITS /
BOUNCE); each finding with severity, file:line and a repro or reasoning; for each numbered item, what you
checked. Paste the pushed commit SHA in the reply to the PL.

**Disk reclaim:** if you built, run `cargo clean` in your worktree afterwards. Never delete the worktree.
