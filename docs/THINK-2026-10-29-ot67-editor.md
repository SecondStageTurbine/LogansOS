<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: OT-67, the text editor

**Date:** 2026-10-29 · **Author:** S1 · **Asked by:** the PM (2026-10-29, while #191 waits on its cross-family read) ·
**Status:** design only, no code. A cross-family read comes before the merge.

The MVP (0.5) needs a text editor (`Plans/DECISION-2026-10-27-release-tiers-mvp.md:10, :20`). OT-67's row
(`docs/OPEN-THREADS.md:77`) says "text-engine THINK first (it gates the app)". This THINK answers:
- whether that gate is met for the MVP editor (§1);
- what the editor is (§2);
- how it stores text, draws it and saves it (§3-§5);
- the prior art (§6);
- the rungs (§7) and J.c.'s decisions (§8).

How the editor fits the app model is in `docs/THINK-2026-10-29-ot88-app-model.md` (#191) §6.1. This THINK builds on
it, and refines its save step (§5.2).

Citations are to `origin/main` at `01b0fc99`.

## 1. Is the text-engine gate met?

**For a plain-text, left-to-right MVP editor: yes.**

- **T0 is built.** neo-text (`userland/neo-text/src/lib.rs:1-11`) is `no_std` + `alloc`. It has:
  - TrueType parsing (`ttf-parser`);
  - exact-area anti-aliased raster;
  - a glyph cache;
  - kerned line layout and word wrap (`layout.rs`);
  - a gamma-correct canvas;
  - a damage-tracking `TextFlow`.

  It ships Inter and IBM Plex Mono.
- **What T0 lacks, and why the MVP editor can do without it:**
  - **Shaping.** `layout_line` maps one `char` to one glyph and adds GPOS pair kerning (`layout.rs:32-46`); there are no
    ligatures and no `ccmp`. For Latin text in Inter and Plex Mono, one char is one glyph. Ligatures would merge
    clusters, which this editor doesn't need.
  - **Cluster tracking.** Because one char is one glyph, the pen position of each char is the cursor position. The
    editor gets grapheme boundaries from `unicode-segmentation` (§6), so a cursor never lands inside a combining
    sequence.
  - **BiDi, complex scripts, font fallback.** These are the text-engine THINK's T2 (§2 "The smallest honest v1 scope";
    `docs/THINK-2026-07-10-atrium-text-engine-research.md:187-211`). They gate correct rendering of document content as well as
    localization (Janus #145 item 1): an English UI can still open a file with combining marks or a script T0
    doesn't shape. The MVP is an explicitly limited left-to-right editor: it **preserves every byte** and draws what T0
    can (§4.1's limits), and doesn't wait for T2.
    The editor's text layer sits behind one trait (§4.1) so T2 can replace it.
- **The gate that is not met:** a document app with styles, rich text and export. That is 1.0's "write a report"
  journey (OT-67's row), and not the MVP's "text editor". This THINK designs the MVP editor; the document app is a later
  THINK (§8, D1).

The row's other lines are stale. "Text output is bitmap glyphs only" and "no text engine" predate neo-text. "Needs
DW-1/DW-2" is answered by #191's app model.

## 2. What the MVP editor is

- **Plain UTF-8 text**, left to right, one document per window.
- **Opens** files Files hands it ("open with", #191 §4.3). Saves back to the same file.
- **Editing:**
  - type, delete, Enter, Tab (a tab character, shown as 4 columns' width of the space advance);
  - move by character, word, line, page, and to the start and end;
  - select with Shift plus any move, by pointer drag, and Select All;
  - cut, copy and paste;
  - undo and redo;
  - find (no replace).
- **View:**
  - soft wrap at the window's width;
  - a cursor and a selection highlight;
  - a status line with the file's name, an unsaved mark, and the line and column;
  - scroll by keys, wheel and drag.
- **Not in the MVP:**
  - styles or rich text;
  - syntax highlighting;
  - several documents or tabs;
  - replace;
  - Save As (§5.3);
  - right-to-left text and complex scripts (T2);
  - files over the size cap (§3.3).

**Keys** (the usual desktop map; nothing modal):

| Key | Does |
|---|---|
| Ctrl+S | Save |
| Ctrl+Z, Ctrl+Y (and Ctrl+Shift+Z) | Undo, redo |
| Ctrl+X, Ctrl+C, Ctrl+V | Cut, copy, paste |
| Ctrl+A | Select all |
| Ctrl+F, Enter / Shift+Enter in find | Find, next, previous |
| Ctrl+Left, Ctrl+Right | Word left, right |
| Home, End; Ctrl+Home, Ctrl+End | Line start, end; document start, end |
| PgUp, PgDn | A page (the visible rows less one, as Arthur's scrollback) |
| Ctrl+W | Close; asks if unsaved |

The keys come as HID usages plus modifiers (`InputEventV1` kind 1), the same as Arthur's (A9-C's decoder). The editor
maps them itself. Nothing reaches it while it isn't in front (#191 §2.3).

## 3. The text buffer

### 3.1 The choice

| | **Gap buffer** | **Rope** (`ropey`) | **Piece table** |
|---|---|---|---|
| What | One `Vec<u8>` with a hole at the cursor; an edit moves the hole | A balanced tree of chunks | The original bytes, plus an append buffer, plus a list of pieces |
| Edit at the cursor | O(1) amortized | O(log n) | O(pieces) |
| Edit far away | O(distance) to move the gap | O(log n) | O(pieces) |
| Line index | Kept separately | Built in | Kept separately |
| `no_std` | Trivial | **ropey is std-only** (`src/lib.rs:198`, `use std::ops::Bound`, and no `no_std` attribute; head below) | Trivial |
| Size in our tree | ~300 lines, ours | A dependency | ~500 lines, ours |

**Recommend a gap buffer of UTF-8 bytes**, plus a line-start index updated on each edit.
- A single cursor, files capped at a few MiB (§3.3), and edits that cluster at the cursor are what gap buffers are best
  at. Kibi and many small editors use one.
- It is `no_std` with no dependency. Every operation is a host KAT.
- The gap always sits on a char boundary. The buffer is valid UTF-8 outside the gap at all times (asserted in debug
  builds and in the KATs).

### 3.2 Undo

A linear history of **edits**: `{at, removed: bytes, inserted: bytes, cursor_before, cursor_after}`.
- Typing coalesces into one edit until a pause in typing, a move, or a word boundary. A paste or cut is one edit.
- Undo applies the inverse; redo re-applies. A new edit after an undo drops the redo tail; there is no undo tree.
- The history is bounded by bytes (1 MiB). Past it, the oldest edits are dropped and the status line says
  "undo history trimmed".
- **Saving does not clear the history.** The unsaved mark compares the current history position with the saved one.

Helix's `Transaction` / `History` (`helix-core/src/{transaction,history}.rs`, MPL-2.0) is the reference for edits as
inverse-able change sets. Its undo is a tree, and ours is a list.

### 3.3 Size cap

The editor opens files up to **4 MiB**. A bigger file gets "this file is too large to edit", never a partial load.
- **Editing is held to the same cap (Janus #145 item 2).** Only the open is checked today: open a 4,194,304-byte file,
  type `x`, and the document and its serialized form are 4,194,305 bytes, which W1 refuses. So every edit is checked
  **before** it mutates the document, against the bytes the file would serialize to (CRLF expansion and a BOM
  included). An edit that would pass the cap is refused whole, with a status-line message ("this file can't get
  any larger"). The core and the app share that one check.
- **Undo history is bounded separately:** 1 MiB of retained edit payload, oldest edits discarded first. One paste
  larger than that is kept in the document but can't be undone, and the status line says so when it happens.
- The buffer, the line index and the wrap cache for 4 MiB fit the editor's address space.
- NeoFS today writes at most 32 KiB through an ordinary write (§5.1), so the cap is set by W1, not by the editor.

### 3.4 What counts as text

The file is opened if **it is valid UTF-8 and has no NUL byte** (checked over the whole file, which the cap bounds).
Otherwise: "this isn't a text file".
- **Line endings:** the editor keeps the file's own convention. LF, or CRLF if every line ending in the file is CRLF.
  A mixed file is read with CRLF counted as one line end and saved with LF, and the status line says so the first time.
- **A UTF-8 BOM** is kept as it was.

This is also the test Files uses to offer "open with the editor" (#191 §4.3), so the two agree.

## 4. Drawing

### 4.1 The text layer

The editor reaches neo-text through one small trait, so T2 can replace it:

```rust
trait TextLayer {
    fn line_height(&self) -> f32;
    /// x of each grapheme boundary in `line`, starting at 0.0.
    fn boundaries(&self, line: &str, out: &mut Vec<(usize, f32)>);
    /// Soft-wrap `para` at `width`: byte ranges of its visual lines.
    fn wrap(&self, para: &str, width: f32, out: &mut Vec<Range<usize>>);
    /// `&mut self`: neo-text's drawing fills its glyph cache (`layout.rs:126`).
    fn draw(&mut self, canvas: &mut Canvas, line: &str, origin: (f32, f32), clip: Rect);
}
```

The T0 implementation (Janus #145 item 1):
- **Wraps by grapheme, not by `char`.** `neo_text::layout::wrap` breaks at char boundaries (`layout.rs:90`), so it
  splits `a` + U+0301 across two rows. The T0 layer wraps with `neo-edit-core`'s grapheme-based wrap
  (`userland/neo-edit-core/src/wrap.rs:29`) and measures with `layout_line`.
- **Expands a tab to four spaces' width**, in `boundaries`, `wrap` and `draw` alike, so the caret, the wrap and the
  pixels agree. T0 itself doesn't expand tabs.
- **Its limits, stated:** no shaping (a combining mark is drawn as its own glyph at the pen position after its base, a
  missing glyph as `.notdef`), left-to-right only. The bytes are never changed by drawing.

The trait hides the font and the shaper, not every later BiDi concern: caret affinity, visual cursor movement and
discontiguous selection rectangles aren't in it, and T2 will extend it.

### 4.2 What's drawn, and when

- **Only the visible rows.** The editor keeps a **wrap cache** per paragraph (a paragraph is text between hard line
  ends): its visual-line ranges at the current width. An edit re-wraps only the paragraphs it touched. A resize clears
  the cache, and paragraphs are re-wrapped as they come into view.
- **Damage:** an edit damages from its first changed visual row to the end of the paragraph. If the paragraph's row count
  changed, it damages to the bottom of the view. A cursor blink damages one cursor rect. Commits use
  `OP_COMMIT_DAMAGE`, as Arthur's do.
- **Selection** is drawn as rectangles behind the text, per visual row, from boundary x to boundary x.
- **The cursor** is a 2 px bar at the boundary x. It blinks on the WM's clock, and not under Reduce Motion (#191 §5).

### 4.3 Pointer

With #191's A1 (pointer to the front client), a press places the cursor at the nearest boundary of the row under it.
- A drag selects. Selecting while dragging past the top or bottom edge scrolls.
- A double click selects a word; a triple click selects a paragraph.
- The wheel scrolls three rows per notch.

## 5. Saving

### 5.1 What NeoFS does today

- An open file has a position (`kernel/src/fs/mod.rs:411-413`). Each `write` is a **whole-file copy-on-write commit**:
  `commit_regular_write` builds the new content and writes it to new blocks (`kernel/src/fs/neofs/inode.rs:555-580`).
- It refuses anything over 8 direct blocks, which is **32 KiB** (`DIRECT_BLOCKS`, `format.rs:16`; `FileTooLarge` at
  `inode.rs:575-577`).
- Larger files exist only through `create_file_from_bytes` (NFS-W1), which uses indirect blocks up to the format's
  2 GiB. It is kernel-internal, behind `neofs-large-write`, used by the installer and a test (`mount.rs:130-158`).

So saving a shorter or longer document today takes two commits: `ftruncate` to 0, then `write` of the whole text. A
crash between them leaves an empty file. (A same-length overwrite is one `write`, one commit; that is the only case
without the empty window, Janus #145 item 4.) And any document over 32 KiB can't be saved at all.

### 5.2 The rung the editor needs: W1, replace a file's content in one commit

**`SYS_FILE_REPLACE(fd, ptr, len)`:** on a file handle with WRITE rights, the file's content becomes exactly `len`
bytes, in **one commit**. A crash leaves the old content or the new, never a mix.

- **It replaces the atomic-save step in #191 §6.1.** That was a temporary file plus `rename_at` with `replace`. With
  W1 no rename is needed, F0's `rename_at` can refuse to replace in every case, and #191 §4.2's `replace = true` exception
  goes. (#191 is amended to match.)
- It needs only the file handle the editor was granted, so it **does not depend on K1b or F0**.

Citations in this section are to `origin/main` at `81d72147` unless stated.

**As built (#209, after #206 merged as `78991d10`).** W1 follows this design, with these differences:
- **One publish, not two.** #206 made every commit free the version it supersedes in that same publish, deferred under
  #180. So step 6 below (a second publish to release the old tree) is gone, and so is §5.2.3's cut "between the publish
  and the release". Truncate lost its second publish the same way.
- **Errors.** No WRITE right is `EACCES`, which is what the file layer already returns. A lost race is `ESTALE`, not
  `EAGAIN`: the handle's generation check catches the second writer before its commit, and the commit's own
  compare-and-swap also maps to `ESTALE`. There is no `ENOSPC`: the kernel has no such errno, so an allocation failure is
  `EINVAL`, as truncate's is. A non-regular file is `EISDIR` (the file layer's `TypeForbidden`). A device failure is
  `EIO`, a new errno (-5).
- **The buffer is checked as user memory** (#209's AAG) before the copy, which runs in 64 KiB chunks with an interrupt
  window between them.
- **The compare-and-swap checks the generation too** (#206's AAG fix U1), not only the inode block, since a freed inode
  block can come back for the same file.
- **Free space.** The scenario runs 50 replaces of 64 KiB, synced every 10, and checks the used count stays constant.
- **A refused replace gives back its reservations** (#213, after #209).

#### 5.2.1 The kernel already has the writer: truncate's

NFS-W1's `create_file_from_bytes` is the wrong base. It creates a *new* name, refuses an existing one
(`inode.rs:290-295`), and streams through a DMA run that bypasses the cache. A replace keeps the name and its directory
entry.

The right base is **`commit_regular_truncate`** (`inode.rs:612-668`). It already does a whole-file copy-on-write at
**any** size up to the format's 2 GiB:
1. it builds the new content in memory;
2. `allocate_file_block_layout` reserves data blocks plus single, double and triple indirect pointer blocks
   (`inode.rs:1088`);
3. it writes them through the cache (`write_content_blocks_cached`, `write_indirect_blocks_cached`);
4. it writes a new inode record into a new inode block;
5. it publishes with **one** `commit_child_update` (`inode.rs:742-765`). That is a compare-and-swap on the directory
   entry: it fails `NotFound` if the entry no longer names the inode block the caller read;
6. then it frees the old tree with `release_old_regular_tree_blocks` (`inode.rs:1205-1229`), under #180's rule that
   freed blocks are not reused until the generation that freed them is durable.

So W1 adds `commit_regular_replace(name, old_inode_block, old_record, bytes)`: truncate's body with `bytes` in place of
"the old prefix, zero-filled". Above it:

| Layer | What W1 adds |
|---|---|
| `NeoInode` (beside `write_at` and `truncate`, `inode.rs:1614`, `:1647`) | `replace(bytes)`. The same rights and kind checks as `truncate`, then the record and inode-block swap after the commit, as both siblings do. |
| `Inode` trait / `OpenFile` (`fs/mod.rs`) | `OpenFile::replace(bytes)`: WRITE right, `validate_current`, then the replace. Afterwards the offset is set to `len` and `refresh_generation` runs, as `write` does (`fs/mod.rs:403-416`). |
| Syscall | `SYS_FILE_REPLACE = 24`, the first free number after `SYS_FTRUNCATE = 23`. The build re-checks it's still free; K1b and F0 add file syscalls too. `len <= MAX_REPLACE_BYTES` (4 MiB, the editor's cap) **before** any copy. Then **`validate_user_buf(buf, len)`: the whole range must be user memory** (`EFAULT`; `copy_user_bytes` trusts its caller for this, and without it a kernel address copies kernel memory into the file). Then the copy, in 64 KiB chunks with an interrupt window between them (the syscall entry runs with interrupts off), then `file.replace` with interrupts on, as `sys_file_write_impl` runs `file.write`. Errors, as built (#209): `EBADF` (not a file handle), `EFAULT` (the buffer is not user memory), `EACCES` (no WRITE), `EISDIR` (a non-regular file), `EINVAL` (`len` over the cap, or no space), `ESTALE` (the handle's generation is old, or the CAS lost to another writer; nothing changed), `EIO` (the device failed). |
| libneo | `sys_file_replace(fd, &[u8])` |

**What it does not do:** stream. The whole new content is in kernel memory once, the copy of the user buffer, and no
second copy is made: the replace consumes the `Vec`. That is why the cap is 4 MiB, not 2 GiB. A larger replace would
stream through NFS-W1's run writer, as a later rung.

#### 5.2.2 Found while reading: ordinary writes never free the old blocks

**Fixed by #206** (merged as `78991d10`): every commit now frees the version it supersedes, and overwrites keep the used
count constant. The text below is the finding as it was written.

`commit_regular_write` (`inode.rs:555-610`) writes the new content and a new inode, publishes, and returns. Unlike
truncate, **it never releases the old content blocks or the old inode block.** The only collector, `run_gc_locked`
(`inode.rs:1343`), is compiled only under the test feature `neo-test-phase3-neofs-3c` (`inode.rs:871`, `:1342`). So
**every `sys_write` to a NeoFS file leaks the blocks the previous version used**, until the volume is full.
- An editor that saved through `ftruncate` plus `write` would leak a file's size on every save. W1 frees the old tree,
  so the editor never takes that path.
- The leak itself is outside OT-67. It goes to the PM as its own finding, a NeoFS fix that applies truncate's release to
  `commit_regular_write`, with a free-block-count scenario.

#### 5.2.3 Byte oracle

**One hand-laid fixture, independent of the writer:** a replace of a 3-block file with **9 blocks** (36 864 bytes:
`0x00..=0xFF` repeated, a pattern the fixture states). That crosses from direct blocks into the single indirect block.
The volume is a fresh small image whose allocator state the fixture fixes, so block numbers are known.

The fixture lays out, by hand, the 256-byte inode record the replace must write (`format.rs:304-323`):
- `0..8`: the magic;
- `8`: kind `Regular`;
- `9`: the type version;
- `12..16`: the generation;
- `16..24`: `file_size = 36 864`;
- `24..32`: `block_count = 9`;
- `32..96`: the eight direct block numbers;
- `96..104`: the single-indirect block number;
- `104..136`: the content BLAKE3 (computed by the fixture's generator from the pattern, not by the writer);
- `136..152`: double and triple indirect, zero;
- `152..156`: the rights;
- `156..160`: the directory entry count, 0;
- `224..256`: the record checksum.

It also lays out the **single-indirect block**: one `u64` little-endian pointer (the ninth data block) followed by
4 088 zero bytes.

The test reads both blocks back off the device after the commit and compares every byte. A second case replaces back
down to 2 blocks: the indirect pointer is zero, and the 9-block tree's blocks are in the deferred-free list at the new
generation.

#### 5.2.4 Crash cuts and the scenario

The QEMU scenario `neofs_file_replace`, built on #180's cut harness:
1. A cut **after each data-block write, after the indirect-block write, after the inode write, and after the publish**.
   On remount, the file reads back exactly the old content (every cut before the publish) or exactly the new content
   (the cut after it). The content hash in the record verifies either way. "After the publish" means after the
   superblock copies are **durable** (the barrier of `cache.rs:613`, :663). A cut after a commit that is only published in
   memory still recovers the old content, and the scenario pins that too (Janus #145 item 5).
2. A cut **between the publish and the release** (the two publishes of §5.2.1, step 5 and step 6). The new content
   survives, and the old tree's blocks are still marked used. That is a leak bounded to one file's old size, the same
   window truncate has today. A test pins that it is a leak and never a double use.
3. **WRITE checked:** a read-only handle gets `EPERM`, and the file is unchanged.
4. **The cap:** `len = 4 MiB + 1` is `EINVAL` before any copy (the kernel heap's high-water mark is unchanged).
5. **A lost race:** two handles on one file, the second replace sees its CAS fail. It returns `EAGAIN`, and the file
   holds the first replace's content.
6. **Free space:** 100 replaces of a 1 MiB file leave the free-block count where one replace left it, once the deferred
   frees are durable. This is the check §5.2.2's bug would fail.

**Kill-switch:** `neofs-file-replace`. Off means no syscall number, no `replace` on the trait, and today's syscall table;
the off build's `.text` equals main's for the kernel's NeoFS objects.

#### 5.2.5 Pitfall-bank pass (`docs/concurrency-pitfalls.md`)

W1 is a new syscall on the NeoFS commit path. It adds no lock, atomic or IPI of its own, but it runs on paths shared
with other CPUs.

| Class | How W1 meets it | Guard and discriminator |
|---|---|---|
| **Lock order on the commit path** | W1 takes truncate's locks in truncate's order: the writeback pause (`cache.pause_worker`), then `entries`, then `metadata` inside `commit_child_update` and `release_old_regular_tree_blocks` | It is the same code path, so the order is inherited, not new. The scenario's 4-CPU run with a concurrent `readdir` loop on the same volume shows no stall (the CP-56 park-not-spin rule holds: these are parking locks, taken with interrupts on) |
| **Lost update between two writers** | Two handles replace one file at once | `commit_child_update`'s compare on the old inode block (`inode.rs:759-761`) makes the loser fail rather than overwrite; scenario item 5 |
| **A reader of the old version** | Another handle holds the old record and reads after the replace | A read that **starts** after the replace sees an old generation, so `validate_current` returns `ESTALE` (`fs/mod.rs:314-322`). **That is not enough for a read already past its check (Janus #145 F4):** `validate_current` (`fs/mod.rs:379`) and `read_at` (:387) are separate, and `NeoInode::read_at` reads blocks from its own snapshot without the allocator's transaction (`inode.rs:1628`). A reader paused after validation can resume after the replace, a durable publish and a reuse of an old block, and read the reuse (its cached verification can skip the rehash, `inode.rs:1858`). Deferred-free durability protects recovery roots, not in-flight readers. **W1 must add a reader rule:** a block freed by a replace is not reused while a read that validated before the replace is in flight (a reader count or epoch on the inode, released at the read's end), or the read re-validates after its block reads and returns `ESTALE`. Discriminator: an armed point pauses a reader after validation; replace, fsync and force a reuse; the reader gets the old bytes or `ESTALE`, never the reused block's. This is an existing shared-path exposure; W1 relies on it, so W1 closes it |
| **Kernel heap under user control** | `len` sizes a kernel allocation | The cap is checked before `copy_user_bytes`, which is fallible (#201). Scenario item 4 |
| **CP-67 / CP-30-shaped teardown** (owned state held across a preemption) | A process killed while in `SYS_FILE_REPLACE` | **"No IPC object" is not a lifetime proof (Janus #145 F3).** A thread preempted Ready inside the replace owns the user-copy `Vec`, the `OpenFile` `Arc`, the layout vectors and a writeback-pause guard (`pause_worker`'s `foreground_writers` count). Today's kill can zombify a Ready thread (`scheduler.rs:317`, :2646) and `Thread::free` frees its stack raw (`thread.rs:243`), so those `Drop`s never run: the pause count stays raised, and the owned memory leaks. `sys_write` has the same exposure. W1 depends on OT-93 (#204, a Ready-in-syscall kill finishes the syscall or unwinds it before the thread is freed) and states its remaining limits. Discriminator: kill a process mid-replace of a 4 MiB file from another CPU, with the thread preempted inside the copy-on-write; afterwards the writeback pause count is zero, the kernel heap and frame counts return, the volume's free count and the handle table are unchanged apart from the commit if it had landed |

**AAG:** a kernel-touching merge, so the standing panel applies. W1 is not SMP-core code (no scheduler, futex or
teardown change), so the cross-family lens is the PM's call, not mandatory. **Muse** reviews the rights check, as
security-relevant Claude-authored code.

### 5.3 New documents without Save As

Save As needs a save-location picker: a WM-drawn powerbox that hands back one new file handle (#191 §6.1). That is a
WM rung of its own.

**Recommend leaving it out of the MVP.** A new document is made in Files: **New text file** creates an empty file in
the current folder (`OPEN_CREATE_REGULAR` works today), asks for its name, and opens it in the editor. The editor only
ever saves to the file it was given. (§8, D3.)

### 5.4 Unsaved changes

- Closing with unsaved changes asks: Save, Don't save, Cancel.
- If the editor is killed (from the Spine) or faults, unsaved changes are lost. There is no autosave or recovery file in
  the MVP, because writing one needs a directory the editor doesn't hold.
- An autosave would ride Save As's picker work later.

## 6. Prior art

How the facts were gathered:
- Each head date and SHA comes from `git clone --depth 1 --filter=blob:none --no-checkout` of the default branch, on
  2026-10-29.
- Each licence was read from the licence file or `Cargo.toml` at that head.
- `no_std` claims were read from each crate's `lib.rs` and `[features]`.
- **No code was copied.** GPL code was read for principle only.

| Project | Licence | Head (date, SHA) | What it is | Fit for us | What we take |
|---|---|---|---|---|---|
| **unicode-segmentation** (`unicode-rs`) 1.13.3 | Apache-2.0 OR MIT | 2026-10-26 (2026-10-27 UTC), `afe6c8e63f62` | UAX #29 grapheme, word and sentence boundaries | **Good.** Always `no_std` (its `no_std` feature is a kept-for-compatibility no-op, `Cargo.toml`) | **A dependency:** cursor stops and word moves |
| **ropey** (`cessen/ropey`) 1.6.1 | MIT | 2026-09-09, `1992665990964` | A UTF-8 rope with line indexing | **Poor.** std-only (`src/lib.rs:198`) | Design reference for the line index |
| **helix** (`helix-editor/helix`), `helix-core` | MPL-2.0 | 2026-09-29, `ba40e547426b` | A modal terminal editor; `helix-core` holds the text model, `Transaction`, `Selection`, `History` | Same licence as ours, so vendorable, but built on ropey (std) | Design: edits as invertible change sets (§3.2). Not vendored: we'd need its rope too |
| **cosmic-text** (`pop-os/cosmic-text`) 0.19.0 | MIT OR Apache-2.0 | 2026-10-27, `b07cb90a872c` | Shaping (harfrust), BiDi, layout, and an `Editor` with cursor and selection (`src/edit/editor.rs`) | Has a `no_std` feature (`src/lib.rs:88-92`), but it pulls harfrust, skrifa, unicode-bidi and more | **The candidate for T2's text layer** (§4.1's trait), as an alternative to rustybuzz inside neo-text. That is the text-engine THINK's Fork 3, not decided here |
| **parley** (`linebender/parley`) 0.12.0 | Apache-2.0 OR MIT | 2026-10-28, `efeabeac2507` | Rich-text layout with a `PlainEditor` | Its default `system` feature does font discovery. `no_std` is partial (`std` is a feature) | Reference for the editor-over-layout split; a T2 alternative too |
| **kibi** (`ilai-deutel/kibi`) 0.3.3 | MIT OR Apache-2.0 | 2026-10-27, `d42f6e34c7d0` | A terminal editor in ≤1024 lines (`README.md`) | std, terminal | The MVP feature set's floor: what a minimal editor still has (find, save prompt, line and column) |
| **xi-editor** (`xi-editor/xi-editor`) | Apache-2.0 | 2023-08-14 (committer date; authored 2023-08-08), `a2dea3059312` | Rope plus CRDT editor core | **Discontinued** (its `README.md`). Read only | Nothing to vendor. Reference for why a CRDT is more than one user needs |
| **lapce** (`lapce/lapce`) | Apache-2.0 | 2026-09-30, `f66ffaa95387` | A GUI editor, xi's successor | std, large | Reference only |
| **zed** (`zed-industries/zed`), `crates/editor` | GPL-3.0-or-later (that crate) | 2026-10-29, `7f9a39a9c083` | A GUI editor | **Principle only** | Nothing |
| **amp** (`jmacdonald/amp`) | GPL-3.0 | 2026-06-09 (2026-06-10 UTC), `df97a3c01e60` | A terminal editor | **Principle only** | Nothing |

**Decision:**
- Write the gap buffer, the undo list and the wrap cache ourselves. They are small, and nothing that fits is `no_std`.
- Depend on `unicode-segmentation`.
- Leave cosmic-text and parley to the T2 decision.

## 7. Rungs

The units are dependencies, iteration cycles and gates (CLAUDE.md).

**Order:**
- E1 and W1 can start now: E1 needs nothing, and W1 needs only file handles. W1's teardown row needs OT-93 (#204) on
  `main` before it merges (§5.2.5).
- **T2 (shaping, BiDi, fallback) is not a prerequisite** of E2 (Janus #145 item 6): E2 ships on the T0 layer with §4.1's
  stated limits. The app-model THINK §6.1 says the same.
- C1 needs #191's A1.
- E2 needs A1, A2, F1 (open with) and W1 (every save goes through it).
- E3 needs C1.

| Rung | What | Depends on | Cost | Close gate | Kill-switch | Kernel? | Muse? |
|---|---|---|---|---|---|---|---|
| **E0** | This THINK and a cross-family read | #191 read | 1 doc, 1 review | PM merge after the review | — | — | — |
| **E1** | `neo-edit-core`, a host-tested `no_std` crate: the gap buffer, the line index, edits and undo, the cursor and selection moves (grapheme and word), find, line-ending handling, the text-file test (§3.4), and the wrap cache over the `TextLayer` trait with a fake layer | — | Low to moderate, mechanical. Many KATs, no gate | Push-CI. Host KATs: random edit sequences checked against a plain `String` model (insert, delete, undo all, redo all ⇒ equal); the buffer valid UTF-8 after every step; cursor stops never inside a grapheme (combining marks and an emoji ZWJ sequence); CRLF round trip byte-exact | Not built into any image yet | No | No |
| **W1** | `SYS_FILE_REPLACE` (§5.2): truncate's copy-on-write path, reached through a file handle | — | Moderate. One new NeoFS function beside truncate, the syscall, and the cut scenario | 4-shard gate: the §5.2.3 byte oracle and the §5.2.4 scenario (crash cuts, rights, the cap, a lost race, free space). **AAG**, with the §5.2.5 pitfall-bank pass | `neofs-file-replace` | **Yes** | Yes (rights check) |
| **C1** | The WM clipboard (#191 §6.2): `OP_CLIP_SET` / `OP_CLIP_GET`, paste only on a paste key the WM delivered | A1 | Low to moderate | 4-shard gate: copy in one client and paste in another; a GET with no paste key delivered is refused; a held client can't GET | `atrium-clipboard` | No | **Yes** (cross-app data) |
| **E2** | `neo-editor`, a Design B client (`MODULE_EDITOR = 7`): drawing (§4), keys, pointer, open from Files, save through W1 (never `ftruncate` plus `write`, which leaks: §5.2.2), close prompt | E1, A1, A2, F1, W1 | Moderate. The drawing and the damage are the cycles | 4-shard gate: open a file from Files, type, save, reopen, and read back the bytes and a digest of the window; the close prompt; a non-text file refused; with the module off, the image unchanged | `neo-editor` module | No | No |
| **E3** | Cut, copy and paste through C1 | E2, C1 | Low | 4-shard gate: copy from the editor and paste into the terminal (or the reverse) | `atrium-clipboard` | No | No |

**What the MVP editor needs:** E1, W1, C1, E2 and E3, plus #191's A1, A2, K1b, F0 and F1. **E1 and W1 can start
before J.c.'s decisions on #191**, because neither depends on D1-D5.

## 8. Decisions for J.c.

**Decided (J.c., 2026-10-29): every recommendation below (D1–D5).**

- **D1. The MVP's editor is a plain-text editor; the "write a report" document app comes later.** **Recommend yes.** **Decided (J.c., 2026-10-29): the recommendation.** The
  document app (styles, headings, export) needs T2 and its own THINK, and is 1.0's journey, not the MVP's.
- **D2. Typeface: Inter (proportional) or IBM Plex Mono?** **Recommend Inter**, **Decided (J.c., 2026-10-29): the recommendation.** with soft wrap: the editor is for
  writing, and the terminal already gives monospace. A mono toggle can come later. Both fonts are already in neo-text.
- **D3. New documents: from Files' "New text file", with no Save As in the MVP?** **Recommend yes** (§5.3). **Decided (J.c., 2026-10-29): the recommendation.** Save As
  needs a WM picker, which is a rung of its own.
- **D4. The size cap: 4 MiB?** **Recommend 4 MiB.** **Decided (J.c., 2026-10-29): the recommendation.** Text documents are far smaller. The cap keeps the buffer and the
  undo bounded.
- **D5. Autosave or recovery after a crash: not in the MVP?** **Recommend not in the MVP** (§5.4). **Decided (J.c., 2026-10-29): the recommendation.** The close prompt
  covers the normal case.

**Open after Janus #145 (item 8), for J.c.:** two behaviours D1–D5 don't settle.
- **D6. Opening a file through a handle without WRITE: a read-only view, or refused?** **Recommend a read-only view**,
  labelled "read-only" in the title band, with Save unavailable. Refusing would hide a file the user can read.
- **D7. An edit that would make the file larger than the cap (§3.3):** **recommend refusing the edit whole**, with a
  status-line message, rather than allowing an unsavable document.

## 9. Which prior-art rules apply

- **The survey rule:** yes (§6).
- **The concurrency pitfall bank:** for W1 only (§5.2). E1, E2 and E3 are single-threaded userland. C1 adds no kernel
  code.
- **Driver trap-mining:** no.
