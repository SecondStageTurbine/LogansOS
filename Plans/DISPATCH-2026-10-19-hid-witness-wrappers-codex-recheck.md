<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Codex re-check of the HID witness wrappers (Jarvis N1) → Jarvis

**Type:** review only. **Deliverable:** one pushed RETURN.

**Why Jarvis:** you raised N1 in `Plans/RETURN-2026-10-19-delivery-drop-fix-crosscheck-jarvis.md` (on
`origin/codex/delivery-drop-fix-crosscheck-jarvis`). The author is Claude-family. This is CP-56's concurrency core:
the IF discipline of the HID locks. CLAUDE.md makes a cross-family lens mandatory for it.

## What is under review

The branch is `claude/hid-if-witness-wrappers`, rebased onto `main` @ `d9296bfb`. The PM names the head SHA when
relaying. Review this diff:

```
git diff origin/main...origin/claude/hid-if-witness-wrappers
```

- **`hid_keyboard.rs`, `hid_mouse.rs`.** `PRIMARY` is taken only through `primary_slot()`: the bind store and every
  `primary()`. The keyboard's `neo-test-hid-mixed-if-control` path keeps its own `lock_as_caller` sample. The
  device lock is taken only through `Keyboard::device()` / `Mouse::device()`. Each wrapper samples the CP-37 IF=1
  witness **before** acquiring. `IrqSafeMutex::lock` masks IF itself, so a sample taken after it always sees IF=0.
  A Claude AAG lens found that the first version, which sampled after, could only catch `lock_as_caller`.
  - `debug_inject_report` is now sampled.
  - The bind-time stores are now sampled.
- **The new control, `neo-test-hid-device-if-control`.** It changes three things:
  - `Mouse::device()` takes the lock with `lock_as_caller`;
  - the mouse's device sections (`device_section`) run without `without_interrupts`;
  - `IrqSafeMutex::lock_as_caller` is compiled for it.

  With these, `hid_contention`'s mixed_if leg must FAIL with `reason=hid_lock_taken_with_interrupts_enabled`.
- **Production:** the claim is no behaviour change. Every section is still IF=0, both from the outer
  `without_interrupts` and from `IrqSafeMutex`.
- **Docs:** `docs/concurrency-pitfalls.md` CP-56.

## Your job: refute it

1. **Completeness.** Is any `PRIMARY` or device-lock acquisition, in either driver or anywhere else, still
   unsampled? Look at `is_bound`, teardown, any IRQ or completion path, and any `try_lock`.
2. **Equivalence.** With no control feature on, does any acquisition, IF state or guard lifetime differ from
   `main`? Check `device_section`, the `impl DerefMut` return, and the guard drop order inside `without_interrupts`.
3. **The control.** Does it fail for the *right* reason, the witness firing? Could it pass while the witness is
   blind? Could it fail some other way, such as a hang, that would hide a blind witness? Does it only ever compile
   into test kernels?
4. **Lock order.** `USB_DEVICE` lockdep still applies through the wrapper.
5. **Coverage.** What does the witness still not see?

A finding bounces the branch only if it survives your own re-verification. "No findings" must state what you checked
for each item.

## How to work

```
WORKTREE: E:\AI Automation\workers\jarvis    — yours alone, never another coder's folder, never the repo root
BRANCH:   codex/hid-witness-recheck-jarvis
          (git fetch origin && git checkout -B codex/hid-witness-recheck-jarvis origin/claude/hid-if-witness-wrappers)
```

```
git fetch origin
git show origin/claude/hid-if-witness-wrappers:Plans/DISPATCH-2026-10-19-hid-witness-wrappers-codex-recheck.md
git diff origin/main...origin/claude/hid-if-witness-wrappers
```

Optional runs:
- `cargo check -Zjson-target-spec --features <f>` in `kernel/` for each of `neo-test-hid-contention`,
  `neo-test-hid-device-if-control`, `neo-test-hid-mixed-if-control` and `neo-test-delivery-drop-control`.
- `cargo xtask neo-test hid_contention` (PASS).
- The same with `NEO_TEST_EXTRA_FEATURES=neo-test-hid-device-if-control` (FAIL,
  `reason=hid_lock_taken_with_interrupts_enabled`).

RAG (semantic code search) is available for this project — use it for conceptual "where is X done?" questions
(faster/more semantic than grep; grep still wins for exact symbols/strings).
Health-check first: `Invoke-RestMethod http://127.0.0.1:8404/health` → expect `{"status":"healthy",...}`.
If down/degraded: the PL manages RAG infra — note it in your RETURN and continue with direct file reads
(never a blocker). Optional self-serve start (coders run PowerShell):
```
powershell -File "E:\AI Automation\RAG\start-rag-ollama.ps1"          # if ollama:false
cd "E:\AI Automation\RAG\AltimitNeo"; .\start-rag.ps1 -SkipIngest     # Qdrant + API
```
Query e.g.: `$body=@{query='HID device lock acquisition IF witness'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`

## Close condition (the RETURN is a pushed artifact)

Commit `Plans/RETURN-2026-10-19-hid-witness-wrappers-codex-recheck-jarvis.md` to your branch and **push it**. It
must contain:
- a verdict per item;
- each finding with its severity, file:line, and a repro or the reasoning;
- for each item, what you checked.

Paste the pushed commit SHA in your reply to the PL.

**Disk reclaim:** this is a review lane. If you built, run `cargo clean` in your worktree afterwards; never delete the
worktree.
