<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH (relay #104): cross-family review of A9-C2, mouse-wheel and touchpad scrolling (#162) → Chad

**MODEL:** GPT-6.1-Sol. **EFFORT:** high. It changes the pointer record ABI and the kernel's pointer route under the
`DELIVERY` IRQ-safe lock (the input queue, the minter and the WM forwarding path), plus both mouse drivers. That's
interrupt/lock code on a shared ABI, Claude-authored (S1). **Do this after relay #100** (keyboard scrolling, which it
builds on).

**Type:** review, post-merge (merge on green, review after, revert-first on a confirmed FAIL).
**Deliverable:** one pushed RETURN. **Reviewed SHA:** `9a1c35bb`, #162's squash on main (PR head `9db35f27` plus a merge of
main). Gate: batch 7, run `40465526041`, all jobs green.

**What it does.**
- **Kind-2 pointer events** carry the wheel in `code`. Today `code` is always 0, so old producers stay valid.
- **The RecordRing** keeps the wheel. A kind-4 record flagged `WHEEL` (0x20) carries it in `x`.
- **Forwarding** yields a new kind-5 event (flags=buttons, code=wheel, x/y local) to the client under the pointer, with
  no grab needed.
- **The drivers.** `hid_mouse` and the touchpad's mouse collection pass the wheel they already decode.
- **The input queue no longer coalesces a wheel report.** Coalescing used to drop it silently.
- **Arthur** scrolls 3 lines per notch.

## Setup

```
WORKTREE: E:\AI Automation\workers\chad     (yours alone — never another coder's folder, never the repo root)
BRANCH:   codex/a9-arthur-wheel-review-chad
          git fetch origin && git checkout -B codex/a9-arthur-wheel-review-chad 9a1c35bb

git show origin/main:Plans/DISPATCH-2026-10-27-a9-arthur-wheel-review-chad.md   # this file
git diff 9a1c35bb^ 9a1c35bb
git show origin/main:docs/concurrency-pitfalls.md                                # CP-37, CP-52, CP-54, CP-56
```

RAG (semantic code search) is available. Health check: `Invoke-RestMethod http://127.0.0.1:8404/health`. If it's down,
note it in the RETURN and use grep/Read; the PL manages it. Example query:
`$body=@{query='pointer route wheel mint_records input queue coalesce'; top_k=5}|ConvertTo-Json;
Invoke-RestMethod -Uri http://127.0.0.1:8404/search -Method POST -Body $body -ContentType "application/json"`.

## Checklist

Answer each item PASS or FAIL with `file:line`. For any FAIL, give a concrete input or interleaving.

1. **ABI compatibility.**
   - The PR's consumer table lists every reader of the 8-byte event, and none rejects a nonzero kind-2 `code`.
   - Check by grep that no reader is missing.
   - The byte oracles for kind 2, kind 4 (WHEEL) and kind 5 are hand-laid and independent of the writer.
2. **The queue change.**
   - A wheel report is never merged or evicted as superseded.
   - Like motion, it never evicts a key or a button transition (a full queue drops it).
   - With no wheel, behaviour is bit-identical to before (the key-only KAT).
   - Kind 3 is untouched.
3. **CP-52 (button transitions and RESYNC).**
   - A WHEEL record is never a transition and never carries RESYNC.
   - A pending RESYNC waits for the next motion record.
   - Give the sequence that would lose a button release, or say why none exists.
4. **CP-37 and the lock.**
   - The wheel is minted and consumed under `DELIVERY` (IrqSafeMutex) at the same sites as today's records.
   - No new lock.
   - No path takes `DELIVERY` with interrupts enabled.
5. **Hover forwarding.**
   - A wheel goes only to the client whose surface is under the pointer, and is dropped otherwise.
   - It can't reach a client that's hidden, behind Home, or locked out.
   - It doesn't change focus.
   - Coordinates are local and clipped.
6. **The drivers.** `hid_mouse` and the touchpad report the sign and magnitude they decode, clamped to the field.
   Check that a report with no wheel still gives `code=0`.
7. **Arthur.** 3 lines per notch, clamped by #159's bounds. `follow` resumes at the bottom.
8. **Kill-switch.** With `atrium-pointer-wheel` / `desktop-wheel` off, the pointer path is today's. The product pairs
   the two features (xtask test).
9. **The scenario.** `desktop_wheel_boot` (wheel=1 → view_top 11, wheel=-1 → follow) would fail if the wheel were
   dropped by the queue or forwarded to the wrong client.
10. **Prior art.** Wayland/Smithay (MIT) and evdev (principle only), with licenses and dates, and nothing GPL copied.

## Close condition

- Commit `Plans/RETURN-2026-10-27-a9-arthur-wheel-review-chad.md` on your branch and **push it**, with PASS/FAIL per
  item and an overall verdict. Paste the SHA to the PL.
- Before ending, check your tree: `git status --porcelain` is empty and `git log @{u}..` is empty.
