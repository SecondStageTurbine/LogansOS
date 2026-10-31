<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family adversarial review of the S1-F1 HID-contention fix → a Codex seat

**Type:** review only (no code to merge). **Deliverable:** a pushed RETURN file.

**Why a Codex seat.** This is concurrency-core: the HID pump on one CPU races a client's RECV on
another for the `DELIVERY` lock. The author and the AAG panel are Claude-family, so CLAUDE.md requires a
different-family refute-lens before merge. The defect itself was found by a Codex seat (Chad, Desktop S1
RETURN, finding 1). The PL assigns the seat (Jarvis / Janus / Chad).

## What is under review

The PR from branch `claude/s1-f1-hid-contention`; the PM names the head SHA in the relay comment. Review the
whole diff: `git diff origin/main...origin/claude/s1-f1-hid-contention`.

**The defect.** `hid_keyboard` / `hid_mouse` called `focus::route_event` per decoded event. `route_event`
takes `DELIVERY` with `try_lock` and returns `false` on contention. By then the driver had already consumed
the USB completion, advanced its decoder and reposted the report slot, so the press or release was gone. A
lost release leaves a client tracking key state stuck.

**The fix**
- Each driver decodes into its own bounded `stage: InputQueue` (64 entries), held under the device's own
  `USB_DEVICE` lock. It uses the same class-aware policy as a client queue: motion coalesces and goes
  first, keys go last.
- At the end of every `drain()`, `focus::flush_staged(&mut stage)` moves every staged event, in order,
  into the active focus queue under ONE `DELIVERY.try_lock()`. Contention moves nothing and loses nothing:
  the events wait for the next pump.
- Two counters are kept apart: `FLUSH_CONTENDED` (busy lock, nothing lost) and `STAGE_DROPPED` (the
  stage's own overflow policy).
- Kill-switch: kernel feature `atrium-input-stage-legacy` restores the direct route.

**The discriminator.** QEMU `hid_contention` (`kernel/src/test_harness.rs` `run_hid_contention`):
- a thread placed on another CPU holds `DELIVERY` almost continuously (tens of µs held, a few
  instructions released);
- meanwhile xtask types 10 letters through a real `usb-kbd` (HMP `sendkey`) and the harness pumps;
- afterwards the holder stops, and every press and release (20 events) must arrive in order;
- non-vacuity: `flush_contended > 0`, and the holder must be on a different CPU from the pump.

## Your job: refute it

Default to suspicious. Try to break:

1. **Loss and duplication.** Is there any remaining path where a consumed report's event is neither
   staged nor delivered (an early `continue`, the fd-0 legacy path, a decoder advanced without staging)?
   Can an event be delivered twice?
2. **Ordering.** Within one device the stage is FIFO. Check:
   - **Across devices:** keyboard and mouse flush independently. Did any guarantee exist before that this
     breaks?
   - **Across a focus handoff:** staged events go to the focus that is active at *flush* time, not decode
     time. `activate()` itself takes `DELIVERY`, so a handoff is a contention source. Is "typed before the
     handoff, delivered to the new focus" acceptable under ADR-074's anti-keylogger / routing contract, or
     is it a security regression? Note that before the fix those keys were simply lost.
3. **Bounds and I-14.**
   - The stage is allocated at bind with capacity 64 and must never grow (len ≤ capacity).
   - Work under `DELIVERY` per flush is at most 64 enqueues, each bounded by the queue policy's
     O(capacity) scan, i.e. O(64²) worst case. Is that hold time acceptable?
   - Under sustained contention, could the stage policy evict a key? (Keys go last; check the incoming-key
     case.)
4. **Pitfall bank** (`docs/concurrency-pitfalls.md`). The PR claims:
   - CP-1: N/A (lock graph unchanged: `USB_DEVICE` → `DELIVERY` via `try_lock` only).
   - CP-2/3: N/A (no IRQ-safe lock, no waiting).
   - CP-11/12/13: N/A (route word untouched; stage state lives under the device lock).
   - CP-14/15/16: N/A (no wake).
   - CP-37: `flush_staged` holds `DELIVERY` with interrupts off whatever the caller's state. The fd-0
     read path drains with IF=1 while RECV pumps with IF=0, and before this change `route_event` took
     `DELIVERY` at IF=1 from fd-0 too.
   - Pre-existing and out of scope, recorded in the PR: the keyboard device lock itself is taken at IF=1
     (fd-0 `try_read_byte`) and at IF=0 (the RECV pump). Is it reachable in a shipped configuration?

   Refute any of these.
5. **Kill-switch.** Is `atrium-input-stage-legacy` equivalent to `main`? The empty-stage flush is a no-op.
6. **Discriminator validity.** Can `hid_contention` pass vacuously (holder co-located, reports counted
   before decode, a too-short window)? The PR shows the control FAILs 4/4 both with the legacy switch and
   on literal `main` (0 of 20 events delivered).
7. **The latches (Jarvis F1/F2, folded into this PR).**
   - **SAK** (`atrium-sak`, `kernel/src/atrium/sak_logic.rs`, `focus::latch_sak`/`take_sak`): can a chord
     be lost, double-latched, or half-delivered (a press or release reaching a client)? Is modifier-first
     right against the real decoder (`sak_kats.rs`)?
   - **Raw buttons** (`focus::raw_buttons`): is it ever stale relative to the delivered stream?
   - **fd-0 now pumps the mouse** (F8), and the keyboard device lock is held IF=0 on that path.
8. **Anything else** a Claude reviewer would structurally miss.

A finding counts only if it survives your own re-verification. Uncertain items go in the RETURN as notes.
"No findings" must state what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\<your seat>     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/hid-contention-crosscheck-<seat>    (git fetch origin && git checkout -B codex/hid-contention-crosscheck-<seat> origin/claude/s1-f1-hid-contention)
```

Read this dispatch and the code from the branch (it is not on `main`):

```
git fetch origin
git show origin/claude/s1-f1-hid-contention:Plans/DISPATCH-2026-10-18-hid-contention-codex-crosscheck.md
git diff origin/main...origin/claude/s1-f1-hid-contention
```

Building and running are optional:
- `cargo xtask neo-test hid_contention` at `NEO_SMP=4` should PASS.
- The same with `NEO_TEST_EXTRA_FEATURES=atrium-input-stage-legacy` should FAIL with
  `reason=hid_reports_lost_under_delivery_contention`.

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?"
questions (faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='HID pump DELIVERY try_lock contention staged flush'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-18-hid-contention-codex-crosscheck-<seat>.md` to
`codex/hid-contention-crosscheck-<seat>` and **push it**. A review delivered only in chat does not exist.
Contents:
- a verdict (MERGE / MERGE-WITH-NITS / BOUNCE);
- each finding with severity, file:line, and a repro or reasoning;
- for each numbered item above, what you checked.

Paste the pushed commit SHA in the reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree afterwards. Never
delete the worktree itself.
