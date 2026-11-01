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

**Revision 2 (2026-11-01), answering the cross-family review** (relay 236,
`Plans/RETURN-2026-11-01-pr12-terminal-think-review-jarvis.md` on `codex/pr12-review-jarvis`): item 1 (flow control
and the term page's ordering) is §3.2's ordering table and the new §3.4; item 2 (a concrete byte oracle) is the new
§5.3, from xterm's `ctlseqs.txt` fetched raw; item 3 (paste) is the new §6.1. Items 4 and 5 passed. Revision 2 also
fixes a bug the review's item 1 led to: revision 1 had the child `FUTEX_WAKE` a terminal that parks in
`SYS_WAIT_ANY`, so a mode change could wait for the next key; the terminal now checks the request on every wake,
at most 20 ms apart.

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
| **L1a.2** | `term-core`, a host-only crate: `vt100` vendored, the key encoder (OT-88 §3.3), the line discipline (§4), the answerback filter (§5.4), the term-page codec. Every part a host KAT, both oracles of §5 | nothing | No | push-CI host tests; no QEMU slot |
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
| stdin pipe, read end | read | Bytes from the terminal: keys (after the line discipline), and answerbacks (§5.4) |
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

**Access and ordering (review item 1).** Both sides touch the page only through 4-byte (or 2-byte) atomic loads and
stores on naturally aligned fields, never through plain references, so neither side's view is a data race. The
orderings are:

| Who | Step | Ordering |
|---|---|---|
| terminal, size | `size_seq += 1` (now odd) | store Relaxed, then a Release fence |
| | columns, rows, width, height | stores Relaxed |
| | `size_seq += 1` (now even) | store Release |
| child, size | `s1 = size_seq` | load Acquire; if odd, retry |
| | the four fields | loads Relaxed |
| | `s2 = size_seq` | Acquire fence, then load Relaxed; if `s2 != s1`, retry |
| child, mode | `mode_req` | store Relaxed |
| | `mode_req_seq += 1` | store Release |
| terminal, mode | `q = mode_req_seq` | load Acquire; if changed since the last one it applied, load `mode_req` (Relaxed), mask, apply |
| | `mode_ack_seq = q` | store Release, then `FUTEX_WAKE` on that word |
| child, mode | wait for `mode_ack_seq >= its q` | load Acquire, `FUTEX_WAIT` on that word between loads |

The size query retries at most 8 times, then returns the last consistent read (the terminal writes the size only on a
configure, so a reader never starves). The seqlock is the standard one, the same pattern as the DI-1b resize
mailbox; L1a.2 has a host KAT that interleaves a writer and a reader at every step of the table above.

**Size:** the terminal writes the size only when the WM configures its window (CP-57-free: no send, no wait).

**Mode change:** the child writes the request as above, then `FUTEX_WAIT`s on `mode_ack_seq` with a deadline (1 s).
The terminal doesn't park on a futex: it parks in `SYS_WAIT_ANY` on its focus, its stdout pipe and a **20 ms deadline**,
and checks `mode_req_seq` on every wake, before it handles any key from that wake. So a mode change is in force within
20 ms whether or not a key arrives, and every key handled after the ack is handled in the new mode. A dead or hung
terminal costs the child one deadline (`TimedOut`), never a hang; the stdout pipe's EPIPE is the liveness signal after
that. The futex wakes across processes (keyed by physical address: OT-88 §3.2, T-b row).

### 3.4 Flow control: pipe sizes, and when a side stops reading (review item 1)

Both pipes are L1a.1's `Handle::Pipe` (OT-88 §3.2, T-a): a bounded byte buffer, written and read as a byte stream with
no framing (a write may be split across reads; a read returns what is there, up to its length).

| | stdin pipe (terminal → child) | stdout pipe (child → terminal) |
|---|---|---|
| Capacity | 4 KiB | 64 KiB |
| Writer | the terminal: **never blocks** (a try-write; a full pipe drops the key and shows the bell, §4) | the child: **blocks** when the pipe is full, as a Unix pipe write does; resumes as soon as the terminal reads |
| Reader | the child: blocks when empty (or waits with a deadline, §3.3) | the terminal: never blocks (it reads only after `SYS_WAIT_ANY` says the pipe is ready) |
| Last other end closes | the child reads the bytes still buffered, then EOF | the terminal reads the rest, then EOF, and shows "[process exited]" |
| Reader side gone | the terminal's write gets EPIPE: it shows the child as gone | the child's write gets EPIPE at once (std: `ErrorKind::BrokenPipe`) |

**The child is slowed by its terminal, on purpose.** A child that writes faster than the terminal draws is held at
64 KiB of output, the same backpressure a Unix terminal gives. It recovers on its own as soon as the terminal reads.

**The terminal drains fairly.** Per wake it reads at most 16 KiB from stdout and parses it, then serves keys and
commits a frame, then goes back to `SYS_WAIT_ANY`. So a child flooding output (`cat` of a large file) can't starve
the keyboard: Ctrl+C and End process are still read within one wake.

**When the terminal itself stops** (hung, or parked by the scheduler): the child's writes block at 64 KiB and its
reads block on an empty stdin; nothing is lost and nothing spins. If the terminal is killed (End from the Spine), its
pipe ends close: the child's next write is EPIPE and its next read is EOF. The terminal also `sys_kill`s its child
on its own exit, so a child never outlives its window unnoticed.

**When the child stops reading:** its stdin pipe fills at 4 KiB; the terminal drops further keys with the bell, never
blocking (§4). Output and End process keep working.

Discriminators (in L1a.1's gate unless marked): a writer filling the pipe parks and wakes when the reader drains 1
byte; EOF after the last writer closes, with the buffered bytes delivered first; EPIPE to a writer after the last
reader closes; and (L1a.3) `terminal_output_flood`: the child writes 10 MiB, and the terminal's End process lands
within one wake while the flood is running.

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
the ignored forms are tested too (§5.5). It is exactly what `neo-shell` and kibi send (kibi's set was read from its
source: CUP, CUU/CUD/CUF/CUB, EL, `?25`, `?1049`, SGR 7 and 38;5, DSR 6n) plus the usual base.

| Class | Sequences |
|---|---|
| C0 | BEL, BS, HT (8-column stops), LF, CR, ESC |
| ESC | `ESC 7` / `ESC 8` (save, restore cursor), `ESC M` (reverse index), `ESC c` (reset), `ESC =` / `ESC >` (keypad) |
| CSI cursor | CUU `A`, CUD `B`, CUF `C`, CUB `D`, CNL `E`, CPL `F`, CHA `G`, CUP `H`, HVP `f`, VPA `d` |
| CSI erase and edit | ED `J` (0, 1, 2), EL `K` (0, 1, 2), ICH `@`, DCH `P`, IL `L`, DL `M`, ECH `X` |
| CSI scroll | SU `S`, SD `T`, DECSTBM `r` |
| SGR `m` | 0, 1, 2, 3, 4, 7, 22, 23, 24, 27, 30-37, 39, 40-47, 49, 90-97, 100-107, `38;5;n`, `48;5;n`, `38;2;r;g;b`, `48;2;r;g;b` |
| Private modes `?h` / `?l` | 1 (DECCKM), 7 (autowrap), 25 (cursor shown), 1049 (alternate screen, save cursor), 2004 (bracketed paste: the mode is tracked and used by paste, §6.1) |
| Reports (§5.4) | DSR `5n`, DSR `6n`, DA1 `c` |

### 5.2 Two oracles

1. **Ours, hand-laid.** A table of `(input bytes, expected grid)` rows: the cells' characters and attributes, and the
   cursor. One row or more per sequence above, plus the edge rows (CUP past the edge clamps; autowrap at the last
   column; DECSTBM then LF at the bottom margin; 1049 round trip restores the main screen and cursor). The expected
   grids are written from **xterm's `ctlseqs`, fetched raw** (`curl` of the upstream text, its URL and date recorded
   beside the table), never from memory or a summarizing fetch. These rows are independent of vt100's code: if vt100
   and our renderer agree with each other but not with ctlseqs, the KAT fails.
   The rows below are the table's first part, written now so the review can check them (review item 2). The rest
   (one or more rows per sequence in §5.1) is L1a.2's first deliverable, in the same form.
2. **Upstream's.** vt100's own fixtures (`tests/data/fixtures/*`, 68 files at `fb0f54e`: input plus expected screen),
   vendored with the crate as MIT test data and run as a second KAT. They test vt100 against itself, so they are a
   regression net for the vendored copy, not the oracle.

**The source.** `ctlseqs.txt` from xterm's maintainer's snapshot repository on GitHub, fetched raw with `git` on
2026-11-01: `ThomasDickey/xterm-snapshots` at `5c75ead9a75d` (2026-10-11), sha256
`364c1c1987c85b1c1135e57a93e9008054f46d09a338de8dcc66ea9a7c613709`. (invisible-island.net, the usual home, is
blocked from this seat's network; the snapshot is the same file.) The line numbers below are in that file. CSI is
`ESC [` = `1b 5b` (l.198-199) and SS3 is `ESC O` = `1b 4f` (l.178-179).

### 5.3 The oracle's first rows (review item 2)

**Key encoder** (the bytes the terminal writes to stdin for a key; xterm's defaults):

| Key | Normal | DECCKM set (`CSI ? 1 h`) | Source |
|---|---|---|---|
| Up, Down, Right, Left | `1b 5b 41`, `1b 5b 42`, `1b 5b 43`, `1b 5b 44` | `1b 4f 41`, `1b 4f 42`, `1b 4f 43`, `1b 4f 44` | l.2471-2479 |
| Home, End | `1b 5b 48`, `1b 5b 46` | `1b 4f 48`, `1b 4f 46` | l.2481-2490 |
| Insert, Delete | `1b 5b 32 7e`, `1b 5b 33 7e` | same | l.2670-2676 |
| PageUp, PageDown | `1b 5b 35 7e`, `1b 5b 36 7e` | same | l.2678-2679 |
| F1, F2, F3, F4 | `1b 4f 50`, `1b 4f 51`, `1b 4f 52`, `1b 4f 53` | same | l.2531-2534 |
| F5, F12 | `1b 5b 31 35 7e`, `1b 5b 32 34 7e` | same | l.2535, l.2542 |
| Shift+F5 | `1b 5b 31 35 3b 32 7e` | same | l.2586 (the file's own example) |
| Ctrl+Right | `1b 5b 31 3b 35 43` | same | modifier code 5 (l.2567-2572), with the `1 ;` prefix; kibi sends exactly this |
| Backspace | `7f` | same | DECBKM reset: "Backarrow key sends delete" (l.1109) |
| Enter, Tab, Esc | `0d`, `09`, `1b` | same | C0 |
| Ctrl+A, Ctrl+C, Ctrl+D, Ctrl+U | `01`, `03`, `04`, `15` | same | C0: the letter's code AND `1f` |

**Grid** (a 10 × 3 screen; cells are row, column from 0; `_` is a blank cell; the cursor is where the next character
goes):

| # | Input bytes | Expected grid (rows 0-2) | Cursor | Checks |
|---|---|---|---|---|
| G1 | `61 62 0d 0a 63` | `ab________`, `c_________`, blank | (1, 1) | CR, LF |
| G2 | `61 62 63 1b 5b 32 44 1b 5b 4b` | `a_________`, blank, blank | (0, 1) | CUB 2, then EL 0 erases from the cursor |
| G3 | `1b 5b 32 3b 35 48 78` | blank, `____x_____`, blank | (1, 5) | CUP is 1-based |
| G4 | `1b 5b 39 3b 39 39 48 7a` | blank, blank, `_________z` | (2, 9), wrap pending | CUP past the edge clamps to the last row and column |
| G5 | `30 31 32 33 34 35 36 37 38 39 41` | `0123456789`, `A_________`, blank | (1, 1) | autowrap at the last column |
| G6 | `1b 5b 37 6d 41 1b 5b 30 6d 42` | `AB________` with A inverse, B plain | (0, 2) | SGR 7, SGR 0 |
| G7 | `61 62 1b 5b 32 4a` | blank, blank, blank | (0, 2) | ED 2 clears and doesn't move the cursor |
| G8 | `61 62 1b 5b 3f 31 30 34 39 68 63 64 1b 5b 3f 31 30 34 39 6c` | `ab________`, blank, blank | (0, 2) | 1049 round trip: the main screen and cursor come back; `cd` stayed on the alternate screen |
| G9 | `1b 5b 31 3b 32 72 61 0a 62 0a 63` | `_b________`, `__c_______`, blank | (1, 3) | DECSTBM 1;2 homes the cursor; LF alone keeps the column; LF at the bottom margin scrolls only rows 0-1 |

**Answerbacks** (written to the child's stdin; the screen is unchanged):

| Input after `61 62` | Bytes written | Source |
|---|---|---|
| DSR 6n `1b 5b 36 6e` | `1b 5b 31 3b 33 52` (`CSI 1 ; 3 R`: row 1, column 3, 1-based) | l.1386-1387 |
| DSR 5n `1b 5b 35 6e` | `1b 5b 30 6e` (`CSI 0 n`) | l.1384-1385 |
| DA1 `1b 5b 63` | `1b 5b 3f 31 3b 32 63` (`CSI ? 1 ; 2 c`, VT100 with AVO) | l.769-772 |

The key encoder's full table (every non-printable key with each modifier, OT-88 §3.3) grows from these rows in L1a.2.

### 5.4 Answerbacks

Three sequences make the terminal write into the child's **own** stdin: DSR `5n` → `CSI 0 n`; DSR `6n` → `CSI row ; col R`
(kibi's size fallback uses it); DA1 → `CSI ? 1 ; 2 c` (VT100 with AVO). Nothing else answers. vt100 hands these to us
as `unhandled_csi` callbacks; the filter is ours, in `term-core`, with a KAT per sequence.

**Not supported, deliberately:** OSC 52 (clipboard read and write: `copy_to_clipboard` / `paste_from_clipboard` are
no-ops), title and icon reports (`CSI 21 t` and friends), DECRQSS, and any other report. These are the classic
"output makes the terminal type" paths; refusing them keeps the only bytes the terminal writes to stdin either the
user's keys or the three fixed answers. Window-title *setting* (OSC 0/2) is ignored in L1a.

Answerbacks obey §4's rule: written non-blocking, dropped if the pipe is full, and at most one per sequence (a flood
of `6n` costs the child its own stdin, nothing else).

### 5.5 The ignored forms are tested

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
   (§5.4), which are fixed strings, not input.
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

- `terminal_unfocused`: with Arthur in front, keys typed reach Arthur and the terminal's child receives nothing; back
  in front, the next key reaches the child. (Routing is the kernel's: a focus that isn't the active route receives
  nothing, `userland/libneo/src/lib.rs:2936-2950`.)

### 6.1 Paste (review item 3)

**L1a has no paste.** The OS has no clipboard yet, the terminal compiles no paste path, and OSC 52 is refused (§5.4).
The only bytes the terminal writes to stdin are the encoder's output for keys it received on its own focus, and the
three answerbacks. So no byte sequence can be "pasted" in L1a, marked or not.

**When a clipboard lands** (a later rung, not L1a), paste is designed now so that rung can't get it wrong:
1. **Paste is a terminal action, not keystrokes.** It starts from a key chord or menu item that arrives on the
   terminal's own focus, so the lock and focus rules above apply to it as to any key. The pasted bytes never go
   through the key encoder or the line discipline's control handling.
2. **The bytes are sanitized before anything else**, by one function in `term-core`: every C0 byte except HT, LF and
   CR is dropped, DEL (`7f`) is dropped, and every C1 code point (U+0080-U+009F) is dropped. CR LF and lone CR become
   LF. So a paste can't contain ESC, and therefore can't contain `ESC [ 2 0 1 ~` (the end marker), `ESC c` (reset),
   or any other sequence; and it can't contain `03` (Ctrl+C) or `04` (Ctrl+D).
3. **In bracketed-paste mode** (the child set `CSI ? 2004 h`), the sanitized bytes are sent as `1b 5b 32 30 30 7e`,
   the bytes, `1b 5b 32 30 31 7e` (ctlseqs l.2821-2828). Without the mode, the sanitized bytes are sent bare.
4. **In cooked mode** the sanitized bytes go into the line buffer as literal text; LF in a paste ends a line, as a
   typed Enter would. Ctrl+U and Ctrl+C inside a paste can't happen, since step 2 removed them.

The sanitizer's KAT ships in L1a.2 even though nothing calls it yet, with the three inputs the review named:

| Pasted bytes | Sent (bracketed mode on) |
|---|---|
| `61 1b 5b 32 30 31 7e 62` (an embedded end marker) | `1b 5b 32 30 30 7e 61 5b 32 30 31 7e 62 1b 5b 32 30 31 7e` (the ESC is gone, so the marker is text) |
| `61 1b 63 62` (`ESC c`) | `1b 5b 32 30 30 7e 61 63 62 1b 5b 32 30 31 7e` |
| `61 03 62` (Ctrl+C) | `1b 5b 32 30 30 7e 61 62 1b 5b 32 30 31 7e` |

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
| `doy/vt100-rust` | `fb0f54e`, 2026-10-19 | MIT | `#![no_std]` with `std` off (`src/lib.rs:36`), `alloc`; deps `vte` (no default features), `itoa`, `unicode-width`; edition 2024 (our toolchain is a 2026-04 nightly, edition 2024). Callbacks for bells, clipboard, title, and unhandled CSI/OSC (`src/callbacks.rs`), which is where §5.4's filter hooks in | **The code, vendored** (parser driver + screen), with its fixtures as test data |
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
