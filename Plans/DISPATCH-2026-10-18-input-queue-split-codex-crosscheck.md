<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family adversarial review of the input-queue key-eviction fix → a Codex seat

**Type:** review only (no code to merge). **Deliverable:** a pushed RETURN file.

**Why a Codex seat.** The change rewrites the overflow policy of the per-client input queue. That queue is shared
state: one CPU's HID pump produces into it while the client's RECV consumes from it on another CPU, under the
`DELIVERY` lock. The author and the AAG panel are Claude-family. This is concurrency-adjacent, so CLAUDE.md
prefers a different-family refute-lens. The PL assigns the seat (Jarvis / Janus / Chad).

## What is under review

The PR from branch `claude/desktop-s2-fonts`. The PM names the exact head SHA in the relay comment. Review the
whole diff: `git diff origin/main...origin/claude/desktop-s2-fonts`.

**The defect** (found in the DI-1 THINK, `docs/THINK-2026-10-18-desktop-di1-pointer-decision.md` §0.2):
- ADR-074 puts keys and pointer events into **one** 64-entry, drop-oldest queue per `InputFocus`
  (`kernel/src/atrium/focus.rs`).
- A mouse flood therefore evicts keystrokes from a client that drains slower than the pointer reports.
- In a multi-window desktop, every client's RECV pumps the shared HID drains, even without focus. A busy focused
  client therefore really can overflow.

**The fix**
- `kernel/src/atrium/input_queue_logic.rs` (new, pure). `InputQueue` keeps the single ordered stream and changes
  only what is merged or evicted:
  1. A motion-only pointer event coalesces into the tail if the tail is also motion-only with the same buttons
     (saturating i16 sum). "Motion-only" means the button state is unchanged since the last pointer event routed
     to this queue.
  2. On overflow, eviction order is:
     - the oldest motion-only event;
     - else an incoming motion-only event is itself dropped;
     - else the oldest button transition that a later queued or incoming pointer event supersedes;
     - else the oldest key.
     Every pointer event carries the full button bitmap, so the final button state always reaches the client.
     The Claude AAG found and fixed a stuck-button regression from evicting an unsuperseded release.
- `enqueue_legacy` is the old rule, selected by kernel feature `atrium-input-queue-legacy` (the kill-switch).
- `focus.rs` switches from `VecDeque<InputEventV1>` to `InputQueue`; `route_event` calls `enqueue`.
- **Host KATs** (`tools/atrium-logic-tests/src/input_queue_kats.rs`):
  - the defect reproduces under the legacy rule;
  - every key survives a random flood, in order;
  - key-only streams are byte-identical to the legacy rule and to an independent drop-oldest reference;
  - tail-only coalescing;
  - saturation and button transitions;
  - the eviction order;
  - a hand-laid wire oracle for a coalesced event.
- **QEMU discriminator** `input_queue_flood` (`kernel/src/test_harness.rs` `run_input_queue_flood`, plus the xtask
  scenario, devices and HMP script):
  - real `usb-kbd` + `usb-mouse`;
  - xtask `sendkey`s 26 characters with a 40-move `mouse_move` burst before each;
  - the harness pumps without draining, then requires every key in order, at least 100 mouse reports, and at least
    one overflow;
  - the control with `NEO_TEST_EXTRA_FEATURES=atrium-input-queue-legacy` must FAIL.

## Your job: refute it

Default to suspicious. Try to break:

1. **Ordering.** Can coalescing or mid-queue removal ever reorder two delivered events? Check key vs. key,
   key vs. click, click vs. click. Can a client observe a button release before its press, or a press with no
   release?
2. **The motion-only test.** `last_buttons` is per queue, starts at 0, and updates on every pointer event,
   including events later coalesced or evicted. Look for:
   - a case where a real button transition is classified motion-only and therefore dropped or merged
     (focus handoff mid-press, a new `RouteEntry`, a queue that was drained);
   - a case where evicting a transition loses state the client needs. The "superseded" rule claims the final
     button state always arrives; a host property test asserts it over 5000 random runs. Break it.
3. **Bounds and I-14.**
   - The producer's work under `DELIVERY` went from O(1) to O(64) (position plus `VecDeque::remove`). Is the
     hold time still bounded and small?
   - Does `VecDeque::remove` or `push_back` ever allocate or grow? `with_capacity(64)` with len ≤ 64 is
     the claim.
   - Does a longer hold raise the `try_lock` contention-drop rate on the producer, which is itself a key-loss
     path? Quantify if you can.
4. **Pitfall bank** (`docs/concurrency-pitfalls.md`). The PR claims:
   - CP-1/2/3: N/A (no new lock, no IRQ-safe lock, no IF-off spin).
   - CP-11/12/13: N/A (the route word is untouched; all queue state stays inside the `DELIVERY` critical
     section).
   - CP-14/15/16: N/A (no wake).
   - CP-37: unchanged guard (`DELIVERY` reached only from process context).
   Refute any of these.
5. **Kill-switch.** Is `enqueue_legacy` truly bit-identical to the pre-fix `pop_front` + `push_back`, including
   the `dropped` counter? Does the key-only equivalence KAT cover what it claims?
6. **Discriminator validity.** Can `input_queue_flood` pass vacuously? Consider:
   - motion not reaching the mouse (QEMU routing);
   - keyboard reports counted before the decode;
   - the settle window too short;
   - `dropped > 0` satisfiable without key pressure.
   Does the control genuinely exercise main's rule? The `atrium-input-queue-legacy` feature calls
   `enqueue_legacy`; confirm it equals main's code.
7. **Residuals the PR names.** Producer `try_lock` contention drops remain (ADR-074 contract), and the
   `RouteEntry` `Vec` is uncapped (pre-existing). Agree these are out of scope, or show one is a blocker.
8. **Anything else** a Claude reviewer would structurally miss.

A finding counts only if it survives your own re-verification. Uncertain items go in the RETURN as notes. "No
findings" must state what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\<your seat>     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/input-queue-crosscheck-<seat>       (git fetch origin && git checkout -B codex/input-queue-crosscheck-<seat> origin/claude/desktop-s2-fonts)
```

Read this dispatch and the code from the branch (it is not on `main`):

```
git fetch origin
git show origin/claude/desktop-s2-fonts:Plans/DISPATCH-2026-10-18-input-queue-split-codex-crosscheck.md
git diff origin/main...origin/claude/desktop-s2-fonts
```

Building and running are optional.
- `cargo test -p atrium-logic-tests input_queue` runs the KATs.
- `cargo xtask neo-test input_queue_flood` should PASS at `NEO_SMP=4`.
- The same command with `NEO_TEST_EXTRA_FEATURES=atrium-input-queue-legacy` should FAIL with
  `reason=keystrokes_evicted_by_pointer_flood`.

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?"
questions (faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='InputFocus route_event per-client queue overflow'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-18-input-queue-codex-crosscheck-<seat>.md` to `codex/input-queue-crosscheck-<seat>`
and **push it**. A review delivered only in chat does not exist. Contents:
- a verdict (MERGE / MERGE-WITH-NITS / BOUNCE);
- each finding with severity, file:line, and a repro or reasoning;
- for each numbered item above, what you checked.

Paste the pushed commit SHA in the reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree afterwards. Never
delete the worktree itself.
