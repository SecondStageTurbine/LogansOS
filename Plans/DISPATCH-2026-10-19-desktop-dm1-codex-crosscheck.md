<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Codex cross-check of DM-1 (desktop motion) → a Codex seat (Chad or Janus)

**Type:** review only (no code to merge). **Deliverable:** one pushed RETURN. **Why a Codex seat:** the author is Claude-family.
DM-1 inserts frames between every state change the WM shows and its present, and widens two syscalls' gating in the kernel. That is
where a same-family review shares blind spots (CLAUDE.md §Cross-model diversity).

## What is under review

Branch `claude/desktop-s1-session-boot`, on `main` @ `35930c52` (DL-1 #57). The PM names the head SHA when relaying. Review
`git diff origin/main...origin/claude/desktop-s1-session-boot`. Design: `docs/THINK-2026-10-19-dm1-motion.md`. Spec:
`userland/neo-atrium/src/motion.rs` (v1.7 "Motion").

**neo-atrium** (`motion.rs`):
- the weight `a(k) = 256 − ⌈256·(N−k)³/N³⌉`;
- a two-bytes-per-lane XRGB blend;
- the Grow / Shrink iris on an anchor;
- `frame()`, which writes only its rect;
- the plans: Full, Reduced (6-frame cross-fades), Off (1 frame).

**WM** (`userland/neo-init/src/desktop_session.rs`):
- **The split.** `present` = `compose` + `commit_present`.
- **`Scene::transition(kind, anchor, rects, client, finals)`.** Motion off: the `finals` presents only. Motion on: `animate` first, then
  the same presents.
- **`motion::animate`:**
  - `F` = the output plane over `rects`, `T` = `compose(client, r)`;
  - frames `1..N−1` blended, then committed and presented rect by rect;
  - pacing on the monotonic clock with `sched_yield`;
  - a pending sprite erase is held back for the end frame.
- **Hook sites:**
  - launch (open + focus);
  - Arthur exit (focus, with the client rect);
  - hold, resume, the Lens view (view);
  - return Home (close);
  - `repaint_lens` (bloom / unbloom);
  - the pointer/keyboard focus switch;
  - Home's chip selection.
- **Boot test only:**
  - `log_motion` (the witness);
  - `hold_point`: the first hold stops at frame 3 until a key on the WM's own focus.
- **Profile:** `/neofs/atrium-reduce-motion` or the `reduce-motion` feature.

**Kernel** (`kernel/src/arch/x86_64/syscall.rs`): `SYS_CLOCK_GETTIME` and `SYS_SCHED_YIELD` (numbers, constants, dispatch arms,
impls) are compiled with `desktop-session` too. Before, they needed `browser-runtime` or `atrium-pointer-forward`.

**Features:**
- `desktop-motion` (neo-init), `reduce-motion`, `desktop-motion-boot-test`, `desktop-motion-rm-boot-test`;
- `desktop-full` now carries motion;
- kernel `neo-test-desktop-motion-boot` / `-rm-boot` (kernel-identical to `desktop_home_boot`).

**Discriminators:**
- **Host:** six `motion_*` KATs in `neo-atrium/src/tests.rs`, against `scene_oracle.py motion-fixture`.
- **QEMU:** `desktop_motion_boot` and `desktop_motion_rm_boot`:
  - the mid-hold capture against the oracle;
  - the end capture against `desktop_lens_boot`'s oracle;
  - each `DESKTOP motion` line held to its plan, bounded damage, pacing and `end_mismatch=0`.
- **QEMU:** `desktop_full_boot`, the same per-transition check.

## Your job: refute it

1. **The end frame.** Is frame `N` really the motion-off present at every hook site?
   - Find a site where `finals` differs from what the code presented before DM-1. Check the pre-DM-1 code on `main`.
   - Find a site where `rects` ⊄ `finals`: a blended frame left on screen outside the end present.
   - Find a site where the base changes between the `T` snapshot and the end present.
   - Is `end_mismatch` a real check, or a tautology? Name a bug it would catch.
2. **Damage.** Can any frame write or present outside its rect?
   - Look at `frame()`, the iris clamp at 0, and a rect clipped to the screen.
   - Can the sprite erase be lost when `animate` errors, or when it returns `None`?
   - Are `F` / `T` stale for a rect not snapshotted?
3. **Kill-switch.** With `desktop-motion` off, are `desktop_lens_*`, `desktop_home_boot`, `desktop_pointer_boot` and
   `desktop_resize_boot` bit-for-bit what they were? Check `move_selection` and `repaint_lens`, which now repaint before
   presenting, and the Arthur-exit site's cfgs.
4. **Kernel.** Does widening the two syscalls' gating change anything for a non-desktop kernel? Can a desktop userland process abuse
   the clock or the yield? Check every cfg site and look for a missed one.
5. **Input and ordering.** Transitions are synchronous inside `hand_over`'s present step.
   - Can a key typed during a transition reach the wrong owner?
   - Can a pointer record be lost?
   - Can the boot-test hold consume a key that was meant for Home or Arthur?
   - The first hold only: what if Arthur exits before any hold?
6. **The oracle.**
   - Is `motion-fixture` independent of the Rust, or a transliteration?
   - Does the mid-hold digest discriminate profiles and frames? Would a wrong frame index pass?
   - Is `HOLD_MASK_BOXES` hiding anything it should not?
7. **Coverage.** What does the gate not cover? The THINK lists:
   - an upper wall-clock bound;
   - RT with motion in QEMU;
   - real silicon;
   - input during a transition.

   Anything else?

A finding bounces the branch only if it survives your own re-verification; uncertain items go in the RETURN as notes. "No
findings" must state what you checked for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\chad    (or E:\AI Automation\workers\janus) — yours alone, never another coder's folder, never the repo root
BRANCH:   codex/desktop-dm1-crosscheck-<seat>
          (git fetch origin && git checkout -B codex/desktop-dm1-crosscheck-<seat> origin/claude/desktop-s1-session-boot)
```

```
git fetch origin
git show origin/claude/desktop-s1-session-boot:Plans/DISPATCH-2026-10-19-desktop-dm1-codex-crosscheck.md
git diff origin/main...origin/claude/desktop-s1-session-boot
```

Optional runs:
- `cargo test -p neo-atrium motion`.
- `python3 scripts/desktop/scene_oracle.py motion-fixture` must reproduce `fixtures/desktop-dm1/motion_digests.rs`.
- `cargo xtask neo-test desktop_motion_boot desktop_motion_rm_boot desktop_full_boot desktop_lens_boot` at `NEO_SMP=4`. Set
  `LIMINE_DIR`.

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?"
questions (faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='WM transition present damage motion frame'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-19-desktop-dm1-codex-crosscheck-<seat>.md` to your branch and **push it**. It must contain:
- a verdict per numbered item;
- each finding with severity, file:line, and a repro or reasoning;
- for each item, what you checked.

Paste the pushed commit SHA in the reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree afterwards; never delete the worktree.
