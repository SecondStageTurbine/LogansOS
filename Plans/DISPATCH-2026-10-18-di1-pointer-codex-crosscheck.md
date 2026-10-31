<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: cross-family adversarial review of the DI-1 pointer + SAK decision → a Codex seat

**Type:** design review only. Nothing to merge, nothing to build. **Deliverable:** a pushed RETURN file.

**Why a Codex seat.** This decision commits ring-0 input-routing code. The recommendation (kernel cursor +
bounded hit-route + SAK chord table) agrees with Oracle's "Strata" THINK. Oracle and this THINK's author are
both Claude-family, so that agreement is same-family echo, and CLAUDE.md §Cross-model diversity rates it weak
evidence. The synthesis (`docs/THINK-2026-07-23-desktop-ux-SYNTHESIS.md`) explicitly deferred this seam to
"its own cross-model adversarial check". Janus, the Codex-family voice on record, hedged toward
userland forwarding. The PL assigns the seat (Jarvis / Janus / Chad). A seat other than Janus gives a fresh
lens; Janus gives continuity with the original Codex position.

## What is under review

`docs/THINK-2026-10-18-desktop-di1-pointer-decision.md` on branch `claude/desktop-di1-pointer-think`. The PM
names the exact head SHA in the relay comment. Read it together with the code it claims to describe, on
`origin/main` @ `7deb10f2` or later.
- **Kernel:** `kernel/src/atrium/{focus.rs, input_logic.rs, logic.rs}`,
  `kernel/src/drivers/usb/{hid_keyboard.rs, hid_mouse.rs, hid_logic.rs, xhci.rs}`, and
  `kernel/src/arch/x86_64/syscall.rs` (input-focus syscalls 0x1A6–0x1AC and their handlers around :6149–6340).
- **WM:** `userland/neo-init/src/desktop_session.rs` and `userland/libneo/src/atrium_desktop.rs`.
- **Docs:** ADR-073, ADR-074, `docs/concurrency-pitfalls.md`.

The THINK's claims, in brief:
- **§0 ground truth:** routing runs in the RECV pump (process context), not in an ISR; keys and pointer share
  one queue; the WM is input-blind.
- **§1:** the SAK table is needed under *both* options, because the WM is keyboard-blind.
- **§2:** Option A (kernel cursor + 16-entry region table + implicit grab + system route) vs. Option B-raw /
  B-ref (WM pointer route + forwarding, by value or by kernel-minted sequence reference).
- **§4:** recommend A under four hard constraints, with falsification test FT-DI1.
- **§5:** the DI-1a rung, its discriminators, and the kill-switch.

## Your job: refute it

Default to suspicious. Try to break each of these:

1. **Ground truth.** Is §0 accurate?
   - Does any path reach `route_event` / `DELIVERY` with IF=0, from the xHCI ISR, or from a timer?
   - Does the RECV pump really run before the permission check, so a bystander's RECV advances everyone's input?
   - Is the shared-queue key-eviction defect (§0.2) real and reachable today?
2. **The "SAK needed under both" claim (§1).** Can a userland design give a keyboard-only user a guaranteed
   escape from a malicious focused client *without* any kernel chord check, while keeping the WM keyboard-blind?
   If so, the framing is wrong.
3. **Option A's ring-0 seam.** For each item, check how it fails:
   - **Table publish vs. pump.** Single writer under `DELIVERY.lock()`; producers use `try_lock`. Look for
     ABBA with `USB_DEVICE` (CP-1), CP-37 exposure, and lock-hold time.
   - **Stale entries.** `focus_id` resolved at publish time vs. a focus dropped later (CP-12), and dead routes in
     the uncapped `Vec<RouteEntry>`.
   - **Grab lifecycle.** A button down in A whose target dies mid-grab; table republished mid-grab; a SAK during
     a grab; a stuck button; several buttons.
   - **Coordinates.** Surface-local values outside the client's size; i16 overflow; framebuffer resize
     (`SYS_FB_OUTPUT_QUERY`).
   - **System-route gating.** Is "holds compositor present authority" (`present_backend::scanout_authorized`)
     the right gate? Can a client obtain a system route or publish a table?
   - **Bounds and I-14.** Do the ≤16 compares, no-alloc and `try_lock` claims really hold? Can a flooding mouse
     or a flooding publisher stall governance?
4. **Option B-ref.** Is route-by-reference really anti-forge? Check:
   - sequence reuse or wrap;
   - forwarding the same event twice;
   - a forwarded click reaching a client after it lost focus;
   - coordinates the WM chooses arbitrarily.
   Is the THINK unfair to B? Does B have advantages it under-weights (smaller ring-0 surface, a Wayland-proven
   model, easier evolution to touch/tablet)?
5. **The A-over-B argument (§4).** The THINK says A wins on "WM never sees client clicks" and "pointer survives a
   wedged WM". Attack both:
   - The WM still sees the cursor position (for the sprite). Is the remaining button-state privacy worth ring-0
     code?
   - With every pixel frozen, is "clicks still route" worth anything?
   - Would you recommend B-ref instead, and on what grounds?
6. **FT-DI1.** Is the falsification test actually able to fail? Name a way A could be wrong that FT-DI1 would
   not catch.
7. **DI-1a rung.**
   - Is the kill-switch truly bit-for-bit (queue split under feature-off)?
   - Do the QEMU discriminators avoid vacuity, e.g. real `usb-mouse` via HMP and not test-inject, and an
     anti-snoop assert that can actually observe a leak?
   - Is anything missing from the pitfall-bank list?
8. **Prior art (§3).** Is any lesson misstated? This is principle only; note anything that would require reading
   GPL code, which is forbidden.
9. **Anything else** a Claude-family reviewer would structurally miss.

A finding counts only if it survives your own re-verification; uncertain items go in the RETURN as notes. "No
findings" must state what you checked for each item. **State your own recommendation (A, B-ref or other) and
its strongest counter-argument.** Your verdict is the cross-family evidence the PL ratifies on.

## How to work

```
WORKTREE: E:\AI Automation\workers\<your seat>     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/di1-pointer-crosscheck-<seat>      (git fetch origin && git checkout -B codex/di1-pointer-crosscheck-<seat> origin/claude/desktop-di1-pointer-think)
```

Read the dispatch and the THINK from the branch (they are not on `main`):

```
git fetch origin
git show origin/claude/desktop-di1-pointer-think:Plans/DISPATCH-2026-10-18-di1-pointer-codex-crosscheck.md
git show origin/claude/desktop-di1-pointer-think:docs/THINK-2026-10-18-desktop-di1-pointer-decision.md
git show origin/codex/desktop-ux-design-vision-oracle:docs/THINK-2026-07-23-desktop-ux-oracle.md
git show origin/codex/desktop-ux-design-vision-janus:docs/THINK-2026-07-23-desktop-ux-janus.md
```

No build is required. If you want to check a claim empirically, `cargo xtask run-desktop --headless --monitor
<port>` boots the live desktop with a real `usb-mouse`, and HMP `mouse_move` / `mouse_button` drive it.

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?"
questions (faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='input focus route_event pointer delivery queue'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-18-di1-pointer-codex-crosscheck-<seat>.md` to `codex/di1-pointer-crosscheck-<seat>`
and **push it**. A review delivered only in chat does not exist. Contents:
- a verdict on the recommendation (AGREE / AGREE-WITH-CHANGES / DISAGREE → B-ref / DISAGREE → other);
- each finding with severity, file:line, and reasoning;
- for each numbered item above, what you checked;
- your own recommendation and its strongest counter-argument.

Paste the pushed commit SHA in the reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree afterwards. Never
delete the worktree itself.
