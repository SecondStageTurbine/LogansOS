<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: L1a, the terminal window on the WM (2026-10-31)

**Status:** design only, no code. For the PM's read, then a cross-family read (it adds a client protocol and a
kernel handle type) before any build is dispatched.
**Asked by:** `Plans/DISPATCH-2026-10-31-seats-resume-on-public.md`, Seat B, item 3.
**Builds on:** `docs/THINK-2026-10-31-lived-in-os-lanes.md` §3 (Lane 1) and §7 (J.c.: a separate terminal window, our
own `neo-shell`), and `docs/THINK-2026-10-29-ot88-app-model.md` §3 (the terminal: vt100, the kernel pipe, key
encoding), whose rungs T1 and T2 L1a *is*. This THINK does not redo OT-88 §3. It settles what §3 left open, and
re-cuts the rungs so the terminal doesn't wait on A2.

Citations are to `origin/main` at `07d7f6e`.

## 1. What L1a is, and where it sits

L1a is the terminal app: a WM client whose window shows a VT text grid, running a child (first `neo-shell`) whose stdin
and stdout are byte streams the terminal owns.

OT-88 §3 already decided most of it:
- **Parser and screen:** vendor `vt100` (which brings `vte`). We write the renderer and the key encoder (§3.1, §3.3).
- **The stream:** a kernel pipe, `Handle::Pipe`, two per terminal (rung T1, §3.2). Its CP table is in §3.2 and stands.
- **The child's stdio is named at spawn,** never by slot; the kernel routes fd 0/1 to a pipe only when the spawn
  selected one (§3.2, "How the child gets its stdio").
- **No signals:** Ctrl+C is `0x03`; End process `sys_kill`s the child (§3.2).

What OT-88 left open, and this THINK settles:
1. the **rung order**: T2 waits on A2 (the Spine chips), which waits on A1c, A1b and A1a (§2);
2. the **client protocol** in full: what the child holds, and how it asks for the size and the mode (§3);
3. **raw and cooked mode**: which is the default, and who switches it (§4);
4. **the escape-sequence subset and its byte oracle** (§5);
5. **the lock screen**: why no key typed while locked can reach the terminal or its child (§6);
6. **the kill-switch** (§7), the gate (§8), and the prior art re-checked today (§9).

## 2. Rungs: L1a.1, L1a.2, L1a.3

OT-88's chain is A0 → A0k → A1a → A1b → A1c → A2 → T2, with T1 beside it. Only two of those links are real for the
terminal:
- **A1a** (the client table, private #220, being re-landed): a second focusable client needs it. Until A1b there is one
  window on screen at a time, which is enough for a terminal.
- **T1** (the pipe).

A2 gives the terminal a Spine chip. It is not needed to *launch* it: the broker's `OP_LAUNCH` with a module id already
launches allowlisted modules (`userland/libneo/src/atrium_launch.rs:22-45`), and a Home intent `terminal` is a
one-line route once the module id exists. So the terminal's dependency on A2 is only for its chip.

**Recommend three rungs:**

| Rung | What | Depends on | Kernel? | Gate |
|---|---|---|---|---|
| **L1a.1** | OT-88 T1 as written: `Handle::Pipe`, the spawn's stdio selection, fd 0/1 routed by type, `SYS_WAIT_ANY` pipe items, plus the term page of §3.2 below | nothing | **Yes** | 4-shard; OT-88 §3.2's CP discriminators; **AAG with a cross-family lens** (teardown, wait queues) |
| **L1a.2** | `term-core`, a host-only crate: `vt100` vendored, the key encoder (OT-88 §3.3), the line discipline (§4), the answerback filter (§5.3), the term-page codec. Every part a host KAT, both oracles of §5 | nothing | No | push-CI host tests; no QEMU slot |
| **L1a.3** | `neo-terminal`, the app: a WM client on A1a's table, rendering `term-core`'s screen through neo-text, spawning `neo-shell` on pipes; `MODULE_TERMINAL = 4` and the Home intent `terminal`; End process | L1a.1, L1a.2, A1a | No (userland) | 4-shard; the scenarios of §8; cross-family read; a security read of §6 |

L1a.1 and L1a.2 can run at once (one kernel, one host-only). L1a.3 needs both plus A1a. A2 later adds the chip
without touching the terminal.

## 3. The client protocol

The terminal has two sides.

**Toward the WM it is an ordinary client.** It uses the surface protocol as it stands: `OP_CREATE_SURFACE`, imported
buffer, `OP_COMMIT_DAMAGE` with vt100's damage rows, `OP_YIELD_FOCUS`. It creates its own input focus and hands the WM
the activator, as Arthur does (`userland/neo-arthur/src/showcase_surface.rs:114-115`). **No new WM opcode.** The one
WM-side change is the module id and its allowlist bit.

**Toward its child it holds three things, all chosen at spawn by the stdio selection (L1a.1):**

| Child holds | Rights | What it is |
|---|---|---|
| stdin pipe, read end | read | Bytes from the terminal: keys (after the line discipline), and answerbacks (§5.3) |
| stdout pipe, write end | write | Bytes to the terminal's parser. stderr is the same pipe (fd 2 resolves to fd 1 today: `userland/neo-std-pal/src/lib.rs:86-95`) |
| term page, a 4 KiB `sys_buffer` | read + write | The size, and the mode request (below) |

The child gets nothing else from the terminal: **no surface channel, no LaunchCap, no input focus, and never the write
end of its own stdin.** That last one is what makes "only the terminal feeds the child's stdin" a fact about handles
(§6).

### 3.1 Why the size and mode go out of band

Anything in the stdout stream is content the child doesn't control: `cat` of a file puts arbitrary bytes there. So
nothing that changes the terminal's behaviour toward input may be an in-band sequence. The size and the mode are
therefore on the term page, written only through the term crate's calls. In-band sequences may change only what the
grid shows (§5).

### 3.2 The term page: layout and byte oracle

Little-endian, fixed offsets, the rest of the page zero. Each word has **one writer**.

| Offset | Size | Field | Writer |
|---|---|---|---|
| 0 | 4 | magic `0x3150_544E` (bytes `"NTP1"`) | terminal, once |
| 4 | 2 | version = 1 | terminal, once |
| 6 | 2 | reserved, 0 | — |
| 8 | 4 | `size_seq`: a seqlock, odd while writing | terminal |
| 12 | 2 | columns | terminal |
| 14 | 2 | rows | terminal |
| 16 | 2 | width in pixels | terminal |
| 18 | 2 | height in pixels | terminal |
| 20 | 4 | `mode_req`: bit 0 cooked, bit 1 echo; other bits 0 | child |
| 24 | 4 | `mode_req_seq`: the child adds 1 after writing `mode_req` (Release) | child |
| 28 | 4 | `mode_ack_seq`: the terminal copies `mode_req_seq` here once the mode is in force | terminal |
| 32 | 4064 | reserved, 0 | — |

**Byte oracle (hand-laid, for L1a.2's KAT):** a page for 80 × 25, 640 × 400 px, cooked with echo, request 1
acknowledged, `size_seq` 2:

```
00: 4e 54 50 31  01 00 00 00  02 00 00 00  50 00 19 00
10: 80 02 90 01  03 00 00 00  01 00 00 00  01 00 00 00
20: 00 ... (4064 zero bytes)
```

The encoder and decoder are tested against these bytes, not only against each other.

**The terminal treats the child-written words as hostile.** It reads `mode_req` once per `mode_req_seq` change, masks
it to bits 0-1, and ignores reserved bits. The child can't hurt the terminal through the page: the terminal never
follows a pointer or a length from it.

**Size:** the terminal writes the size under the seqlock (CP-57-free, the DI-1b resize pattern); the child's size query
retries on an odd or changed `size_seq`, bounded at a few tries before returning the last good read.

**Mode change:** the child writes `mode_req`, bumps `mode_req_seq`, `FUTEX_WAKE`s the terminal's word, and
`FUTEX_WAIT`s on `mode_ack_seq` with a deadline (1 s). The terminal applies the new mode **before the next key it
handles** and then publishes the ack. A dead or hung terminal costs the child one deadline (`TimedOut`), never a
hang; the stdout pipe's EPIPE is the liveness signal after that. The futex wakes across processes (keyed by physical
address: OT-88 §3.2, T-b row).

### 3.3 What the child's side looks like

- **libneo:** `libneo::term::{size(), set_mode(Mode), is_terminal()}` on the term page; `is_terminal` is false when the
  spawn selected no term page.
- **std:** a `std::os::neoplutarch::term` extension with the same three calls, plus `std::io::IsTerminal` answering
  from `is_terminal`. termion's backend (for later apps) maps `terminal_size` and raw mode onto them. kibi's Unix
  path needs exactly these (`TIOCGWINSZ`, `cfmakeraw`) and a read that times out (VMIN/VTIME) to tell a lone Esc from
  a sequence: `SYS_WAIT_ANY` on the pipe item with a deadline gives that (L1a.1).

## 4. Raw and cooked mode

**Recommend cooked with echo as the default, a change from OT-88 §3.2 (which made raw the default).** The reason: a
plain std program that calls `stdin().read_line` and never asks for a mode should behave as on any OS: see what it
types, edit with Backspace, deliver on Enter. Programs that want raw ask for it: `neo-shell` (it edits its own line,
`userland/neo-shell/src/main.rs:1119-1142`), kibi, any TUI. Raw by default would leave every simple std program with no
echo.

The line discipline is the terminal's, in `term-core` (host-tested), not the kernel's and not the child's:
- **cooked:** printable bytes go into a line buffer (4096 bytes; beyond that, a bell and the byte is dropped) and are
  echoed into the parser; Backspace removes the last UTF-8 scalar; Ctrl+U clears the line; Enter appends `\n` and
  writes the line to the stdin pipe; Ctrl+D on an empty line closes the pipe's write end (EOF), on a non-empty line
  delivers it without `\n`.
- **raw:** the encoder's bytes go straight to the pipe.
- **both:** Ctrl+C writes `0x03` (raw) or discards the line and writes `0x03` (cooked). Ctrl+Shift+C is End process,
  handled by the terminal, never sent.

**The terminal never blocks on its child.** Its pipe writes are non-blocking; a full stdin pipe drops the key and rings
the bell (visual). So a child that stops reading can't stall the terminal's window, and the terminal never parks in a
send, the CP-57 shape.

Why not put cooked mode in the child's std instead: it would need no mode protocol, but type-ahead would not echo
while the child is busy, and every no-std child would need its own copy.

## 5. The escape-sequence subset and its byte oracle

### 5.1 What we commit to

vt100 parses more than this. The list is what L1a promises and tests; anything else is ignored (vt100's behaviour), and
the ignored forms are tested too (§5.4). It is exactly what `neo-shell` and kibi send (kibi's set was read from its
source: CUP, CUU/CUD/CUF/CUB, EL, `?25`, `?1049`, SGR 7 and 38;5, DSR 6n) plus the usual base.

| Class | Sequences |
|---|---|
| C0 | BEL, BS, HT (8-column stops), LF, CR, ESC |
| ESC | `ESC 7` / `ESC 8` (save, restore cursor), `ESC M` (reverse index), `ESC c` (reset), `ESC =` / `ESC >` (keypad) |
| CSI cursor | CUU `A`, CUD `B`, CUF `C`, CUB `D`, CNL `E`, CPL `F`, CHA `G`, CUP `H`, HVP `f`, VPA `d` |
| CSI erase and edit | ED `J` (0, 1, 2), EL `K` (0, 1, 2), ICH `@`, DCH `P`, IL `L`, DL `M`, ECH `X` |
| CSI scroll | SU `S`, SD `T`, DECSTBM `r` |
| SGR `m` | 0, 1, 2, 3, 4, 7, 22, 23, 24, 27, 30-37, 39, 40-47, 49, 90-97, 100-107, `38;5;n`, `48;5;n`, `38;2;r;g;b`, `48;2;r;g;b` |
| Private modes `?h` / `?l` | 1 (DECCKM), 7 (autowrap), 25 (cursor shown), 1049 (alternate screen, save cursor), 2004 (bracketed paste: accepted, nothing pastes yet) |
| Reports (§5.3) | DSR `5n`, DSR `6n`, DA1 `c` |

### 5.2 Two oracles

1. **Ours, hand-laid.** A table of `(input bytes, expected grid)` rows: the cells' characters and attributes, and the
   cursor. One row or more per sequence above, plus the edge rows (CUP past the edge clamps; autowrap at the last
   column; DECSTBM then LF at the bottom margin; 1049 round trip restores the main screen and cursor). The expected
   grids are written from **xterm's `ctlseqs`, fetched raw** (`curl` of the upstream text, its URL and date recorded
   beside the table), never from memory or a summarizing fetch. These rows are independent of vt100's code: if vt100
   and our renderer agree with each other but not with ctlseqs, the KAT fails.
2. **Upstream's.** vt100's own fixtures (`tests/data/fixtures/*`, 68 files at `fb0f54e`: input plus expected screen),
   vendored with the crate as MIT test data and run as a second KAT. They test vt100 against itself, so they are a
   regression net for the vendored copy, not the oracle.

The key encoder's oracle is OT-88 §3.3's (every non-printable key with each modifier, from the same raw ctlseqs).

### 5.3 Answerbacks

Three sequences make the terminal write into the child's **own** stdin: DSR `5n` → `CSI 0 n`; DSR `6n` → `CSI row ; col R`
(kibi's size fallback uses it); DA1 → `CSI ? 1 ; 2 c` (VT100 with AVO). Nothing else answers. vt100 hands these to us
as `unhandled_csi` callbacks; the filter is ours, in `term-core`, with a KAT per sequence.

**Not supported, deliberately:** OSC 52 (clipboard read and write: `copy_to_clipboard` / `paste_from_clipboard` are
no-ops), title and icon reports (`CSI 21 t` and friends), DECRQSS, and any other report. These are the classic
"output makes the terminal type" paths; refusing them keeps the only bytes the terminal writes to stdin either the
user's keys or the three fixed answers. Window-title *setting* (OSC 0/2) is ignored in L1a.

Answerbacks obey §4's rule: written non-blocking, dropped if the pipe is full, and at most one per sequence (a flood
of `6n` costs the child its own stdin, nothing else).

### 5.4 The ignored forms are tested

KAT rows for: an unknown CSI final byte; OSC 52 set and query (no bytes out, grid unchanged); a CSI with 40
parameters (vte caps them; the grid is unchanged); a truncated sequence at the end of a write followed by its rest in
the next write (same grid as one write); invalid UTF-8 (U+FFFD cells).

## 6. The lock screen: no input reaches the terminal while it is up

**The property:** no key typed while the session is locked reaches `neo-terminal` or its child.

It holds by construction, from three facts, each with its discriminator:

1. **The terminal's only input is its own focus.** Keys reach it only through `sys_input_focus_recv` on its own focus
   (`userland/libneo/src/lib.rs:2942`). While locked, the kernel's lock route (L2b, `sak-lock-route`, the CP-97 guard:
   `docs/concurrency-pitfalls.md:143`) makes every client receive fail and sends every event to the WM's lock focus,
   and nothing a client's queue held across the lock survives the unlock (the lock epoch). The terminal is a client,
   so it inherits this. It adds no other input source: no clipboard, no paste, no network, no second focus.
2. **Only the terminal writes the child's stdin.** The child holds the stdin pipe's read end only (§3). The terminal
   never grants the write end, and its only writes there are its line discipline's output and the three answerbacks
   (§5.3), which are fixed strings, not input.
3. **The lock chord never reaches either.** Super+L is a kernel SAK chord, taken in the latch before any route
   (`userland/libneo/src/lib.rs:3067-3100`). A child can't see, fake or swallow it.

What *is* allowed, and is not a leak: a partial line typed before the lock stays in the terminal's cooked buffer, and
lines completed before the lock are already in the pipe. Those were typed by the signed-in user before locking. The
child's output keeps arriving while locked, and the WM accepts the terminal's commits but composites nothing but the
lock (`userland/neo-init/src/desktop_session/lock.rs:12-21`), so nothing the child prints shows under the lock.

**Discriminators (L1a.3's gate):**
- `terminal_lock_park`: in the terminal, cooked mode, the child echoing what it reads. Type `ab`, Super+L, type
  `x` + Enter (they go to the lock's password field), unlock, type `c` + Enter. **The child receives exactly `abc\n`.**
  A leg with the WM parked in a send (the `login_lock_park` shape) gives the same. **Falsified** by a control build
  with the kernel lock route off and the WM's own swap skipped: the child receives the `x`.
- `terminal_stdin_write_refused`: the child writes to its stdin handle; the write fails on rights, and the terminal's
  grid and the child's next read are unchanged.
- `terminal_osc52_inert`: the child prints an OSC 52 query; nothing arrives on its stdin.

Both §6 and the client protocol go to the cross-family security read before L1a.3 merges.

## 7. Kill-switch

- **L1a.1:** feature `pipe` off ⇒ no pipe handle type, no stdio selection, no term page; today's syscall table. With it
  on and no spawn selecting a pipe, every existing scenario's captures are unchanged (OT-88 §3.2's leg, including
  Arthur's spawn with channels in slots 0 and 1).
- **L1a.2:** a host crate nothing links until L1a.3. No switch needed; no image change.
- **L1a.3:** the `neo-terminal` module off ⇒ not in the image, no module id in the allowlist, no Home intent; every
  desktop scenario's captures are unchanged. The WM gains no code path with it off.

## 8. The gate for L1a.3

On the hosted 4-shard gate:
- `terminal_shell_echo`: launch the terminal from Home, type `echo hi` + Enter into `neo-shell`; the grid read back
  shows `hi` on the next row and the prompt after it.
- `terminal_escape_grid`: the child writes a fixed script of §5.1 sequences; the grid read back equals the oracle's
  grid for that script (the same rows as L1a.2's KAT, run through the real renderer).
- `terminal_size`: the child's size query returns the window's columns and rows; after a configure to a new size,
  the next query returns the new one.
- `terminal_mode`: a child that sets raw mode reads `a` the moment it is typed, with no echo; after it sets cooked,
  `b` echoes and arrives only at Enter.
- `terminal_ctrl_c_end`: Ctrl+C reaches a child that reads it; a child that ignores it is ended by End process, the
  terminal shows the exit and lives on.
- `terminal_child_dies`: the child exits nonzero; the terminal shows it and the WM and Arthur live on.
- `terminal_stalled_child`: the child stops reading; 5000 keys are typed; the terminal keeps repainting and the WM
  serves Arthur throughout.
- §6's three discriminators.
- With `neo-terminal` off, every desktop scenario's captures unchanged.

Scenario timeouts are set from a measured hosted run (the `desktop_home_cycle` lesson, main `76a1f03`).

## 9. Prior art (re-checked 2026-10-31)

Heads read by `git clone --depth 1` today; licences from each repository's licence files and `Cargo.toml`.

| Project | Head | Licence | Fit | What we take |
|---|---|---|---|---|
| `doy/vt100-rust` | `fb0f54e`, 2026-10-19 | MIT | `#![no_std]` with `std` off (`src/lib.rs:36`), `alloc`; deps `vte` (no default features), `itoa`, `unicode-width`; edition 2024 (our toolchain is a 2026-04 nightly, edition 2024). Callbacks for bells, clipboard, title, and unhandled CSI/OSC (`src/callbacks.rs`), which is where §5.3's filter hooks in | **The code, vendored** (parser driver + screen), with its fixtures as test data |
| `alacritty/vte` | `89c9c22`, 2026-10-25 | Apache-2.0 OR MIT | `no_std` without `std` (`src/lib.rs:30`); deps `arrayvec`, `memchr` (both already in our `Cargo.lock`) | **The code**, as vt100's dependency |
| `redox-os/termion` | `c784cec`, 2025-11-21 | MIT | Raw mode and ANSI helpers for apps; its Unix backend expects termios. About a year since the last commit: alive, not busy | **A LogansOS backend** on §3.3's calls, for later apps; nothing in L1a links it |
| `ilai-deutel/kibi` | `d42f6e3`, 2026-10-27 | MIT OR Apache-2.0 | L1b's editor; per-OS backends (`src/unix.rs`, `src/wasi.rs`, `src/windows.rs`) | Nothing in L1a. Read to fix the subset (§5.1) and the size/raw/timeout needs (§3.3), so L1b is a backend file, not a terminal change |

Not re-checked here, and why: OT-88 §9 and `Plans/SURVEY-2026-10-27-ot88-terminal-emulator.md` cover the wider field
(alacritty_terminal, wezterm's term crate); vt100 was chosen there. Archived status was not checked (no API from this
seat); L1a.2 re-checks it on the day it starts, with the heads.

Two checks are left to L1a.2's first step, as OT-88 §3.1 said: vt100 builds on our userland target with `std` off,
and its scrollback model. (A host build of vt100 was not run from this seat.)

## 10. Questions for the PM

1. **The rung cut (§2):** L1a.3 depends on A1a, not A2, with the Home intent as its launch path until A2's chip.
   Recommend yes.
2. **Cooked by default (§4),** changing OT-88 §3.2's raw default. Recommend yes.
3. **Seats:** L1a.1 is kernel work with a cross-family AAG; L1a.2 is host-only and can go to a no-gate seat now. Seat B
   can take L1a.2 next, or L1a.1 if the PM prefers the kernel rung first.

No J.c. decision is needed: the product call (a separate terminal window) is made, and these are design questions.
