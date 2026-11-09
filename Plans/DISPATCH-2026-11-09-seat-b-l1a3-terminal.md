<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: Seat B, L1a.3 — `neo-terminal`, the terminal window (2026-11-09)

**Seat:** Seat B (cloud). **Branch:** `claude/l1a3-terminal` off `origin/main` (one branch per PR).
**Design:** `docs/THINK-2026-10-31-l1a-terminal-window.md` (§2 rung L1a.3, §3 client protocol, §3.3 flow control,
§4 raw/cooked, §6 lock screen, §7 kill-switch), with `docs/THINK-2026-11-08-l1a1-stdio-selection.md` (§2.4 how the
child finds its stdio and term page).

## Why now

Everything L1a.3 needs is on `main`: the kernel pipe and stdio chosen at spawn (L1a.1, #46, the 2a squash, #72 as
`9f175301`), `neo-term-core` (L1a.2, #42) and the WM client table (A1a, #15). This rung is the first thing a user
sees from the whole L1a lane: a window that runs `neo-shell`.

## What to build

1. **`neo-terminal`**, a WM client on A1a's table. It renders `neo-term-core`'s grid through neo-text, spawns
   `neo-shell` with the stdio selection (fd 0/1 on two pipes, the term page), and pumps key events through the key
   encoder and the line discipline into fd 0.
2. **`MODULE_TERMINAL = 4`** and the Home intent that opens it (THINK §2's table).
3. **Flow control per §3.3.** The terminal never blocks its event loop on a pipe. A child that floods its output
   can't starve input, and End process still lands.
4. **The lock screen per §6.** No input reaches the terminal while the lock screen is up.
5. **Kill-switch per §7:** with the feature off, the image is byte-identical to today's. Prove it with the
   `.text`/size check or a hash, not by inspection.

## Gate

- Scenarios as the THINK names them. At minimum: the window opens and `neo-shell`'s prompt renders; typed input
  round-trips (cooked echo); `terminal_output_flood` (10 MiB, End process lands); lock screen up means no input
  delivered; kill-switch off means no terminal module.
- `scripts/worker-close-gate.sh`, then a green 4-shard PR run at the head. Paste the URL in a PM: comment.
- The size gate (#3/#96): if the terminal grows image A past a ceiling, post the measured numbers. Don't raise a
  ceiling without the PM.

## Prior art

The parser comes from the vendored `vt100` (L1a.2), so no new third-party code is expected. If you take in anything
new (a font path, a clipboard piece), the PR carries the survey table: license, last commit date and SHA.

## Concurrency

The terminal's pump touches pipe waits and the WM channel. State in the PR, per `docs/concurrency-pitfalls.md`
class, whether the change introduces, guards or leaves each one alone, with the discriminator. The wait-item
lost-wakeup and pipe park/wake classes apply.

## Review

When it's green, the PM queues a cross-family review. §6 (lock screen) and the client protocol also need the
cross-family **security** read before merge (THINK §6, last line). That read goes to Muse (Gwen), since the code is
Claude-authored. Say in the PR which commits it should read.

## Close

Post PM: on the PR with the run URL, the size numbers and the kill-switch proof. Commit and push at every
checkpoint. Keep at most two gates in flight. Once it lands, delete the branch.
