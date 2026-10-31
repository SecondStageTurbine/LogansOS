<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN: Codex cross-check of desktop consolidation (Janus)

**Commit reviewed:** 02cff42adf0c2bb63961860ebb6a5bb8fae4cc76 (#63)

**Base:** e65da2d91dba87c4219dcf47b39a130301cc25a4

**Verdict:** **PASS — no findings.** I found no behavioral, pixel, serial-log,
feature-closure or scenario-driver divergence from the parent. The consolidation
is an exact-equivalence cleanup as claimed.

## 1. WM equivalence — PASS

I compared every extracted transition call with its parent inline block:

- launch window: Open, view rects, union view damage, final union present,
  client on;
- launch focus: Focus, focus rects, final focus damage, client on;
- resume: View, the same view regions/final, client on;
- hold: View, the same view regions/final, client off;
- return Home: Close, the same view regions/final, client off;
- pointer focus/Lens selection: Focus, the same focus regions/final, client on;
- Lens view: View, the same view regions/final, client on.

The helpers evaluate the former local expressions and call transition with
them in the same order. With motion off, transition still performs only the
finals presents in slice order; with motion on, its plan, rectangles, anchor
and unchanged end present are the same. No site has an intervening geometry
mutation between the former captures and the helper call.

grant_over preserves the short-circuit order offer O -> receive R -> grant ->
receive A. Rights are unchanged: full channel rights for surface/LaunchCap
endpoints, USE|(MAP_READ|QUERY) for the configure mailbox, and
USE|(MAP_WRITE|QUERY|COMMIT) for the Lens buffer. Caller mappings remain 8
(main surface), 82 (LaunchCap), 97 (Lens surface), 94 (mailbox), and 99
(Lens buffer).

I compared all manual-buffer-to-Line conversions: client damage, flat-layout
geometry, launch verdict, Home discard, view residue/stale counts, Home ASCII,
pointer press/select/action/forward, Lens failure, and Lens-view exit. Line::n
emits the same unpadded unsigned decimal. Line::i emits the same optional minus
plus unsigned_abs magnitude as put_i64. Home ASCII remains variable-width
exactly as before. Separators and newlines are unchanged.

handle_commit now discards its unused serial internally and returns unit; its
only caller already inspected only success/error, so protocol order and error
codes are unchanged.

## 2. Removed/narrowed cfg gates — PASS

- pointer_tick changed from pointer-and-Home to pointer; desktop-pointer
  implies desktop-home.
- repaint_reframed's assertion lost an inner Home gate, but the function is
  pointer-gated and pointer implies Home.
- Line is no longer glass-gated because the flat S1 layout log now uses it;
  it replaces manual formatting with byte-identical formatting.
- log_stale_outside gained the boot-test gate already present at all callers.
- Proof::Late no longer needs the non-pointer dead-code allowance.
- cfg! for window_fixed equals the former mutually exclusive declarations:
  only Lens-without-resize can make it true.

Clippy with -D warnings passed for neo-init under desktop-session,
desktop-glass, desktop-home, desktop-pointer, desktop-lens,
desktop-lens-pointer, desktop-lens-fault-test, desktop-resize, desktop-full,
desktop-motion, desktop-motion-boot-test, and desktop-motion-rm-boot-test.

## 3. Kernel features — PASS

The effective dependency closure of every scenario is unchanged:

- S1/glass/glass-RT are untouched;
- Home/Lens/Lens-fault/motion/motion-RM resolve to neo-test + desktop +
  neo-test-atrium as before;
- pointer/Lens-pointer/resize/full resolve to that Home closure plus
  atrium-pointer-forward as before.

Descendants now depend on non-scenario bases instead of inheriting another
scenario's boot discriminator. Each CI-built scenario therefore enables
exactly one SCENARIO and one DESKTOP_PASS; none enables two or zero. All eleven
desktop pass strings moved byte-for-byte. Widened Home cfgs cover exactly the
family formerly identified transitively by the Home boot feature; no such cfg
use remains elsewhere in the kernel with a changed meaning.

## 4. xtask table — PASS

I compared every row to the removed arms:

- session/glass/glass-RT retain the session script, own end capture, capture
  after newline, ordinary timeout, and non-Home band check;
- Home retains its script/capture and Home-flow band check;
- pointer retains its pointer script, Home capture, and no band check;
- Lens retains its script/capture, capture after carriage return, and Home-flow
  band check;
- Lens-fault retains the Lens script, fault capture, carriage-return capture,
  and no band check;
- Lens-pointer retains its click script/capture and exact 06-03-04 command;
- resize retains its script, Home capture, newline capture, and no band check;
- full retains its script/capture, exact 1a-1a-03-04 command, and 300s timeout;
- motion and motion-RM retain the shared script, distinct end/mid captures,
  carriage-return capture, Home-flow band checks, and 300s timeouts.

All twelve retain 2048M, pinned 1920x1080 and qemu max. Table names are unique
and match the twelve desktop TEST_SCENARIOS entries. Its early lookup matches
only those exact names, so no non-desktop timeout, memory, CPU or serial arm is
shadowed. Shared HID constants copy the old prompt/command bytes exactly.

load_capture, rgb and masked_fnv preserve 1920x1080 validation, row-major
(y,x) traversal, mask membership, RGB to 0x00RRGGBB, four little-endian bytes
per pixel, FNV offset cbf29ce484222325 and prime 100000001b3. read_serial and
require_single_arthur preserve the former reads, counts and error branches.

## 5. neo-atrium helpers — PASS

- Rect::grow is the former x-n, y-n, w+2n, h+2n at every call site.
- chip_label_of, docked_group and FIT_PAD substitute identical expressions.
- round_rect forwards identical coordinates/radius; blur_rect supplies the
  same backdrop closure, dimensions and rectangle tuple.
- glow_mix retains the zero-alpha no-op and the same Electric Blue mix in the
  same pre-panel order.
- title_text retains label x+4, baseline y+18, 14px name, measured width plus
  8px, and 11px cue for both Arthur and Lens.
- Named panel indices replace the same numeric indices; field frost is the
  same previously computed Owned value.

The test helpers preserve the former PPM bytes, channel loops, thresholds and
tuple order. Negative controls remain. cargo test -p neo-atrium passed 48
tests with 2 ignored.

## 6. Proof — PASS

Using a dedicated scratch directory, all 14 files under fixtures/desktop-*
regenerated byte-for-byte. On this Windows seat I used a temporary ignored
shim in target/ to invoke the installed Python with numpy/fontTools, UTF-8
output and LF normalization; no tracked checker file was changed.

I then changed one hex digit in desktop-s3/scene_digests.rs. The checker
reported it as DIFF, regenerated the other 13 as same, and exited 1. I restored
the byte and verified no tracked diff remained.

The script has 14 explicit check lines and there are 14 one-level desktop
fixture files. A generator error, empty output, or missing portion fails cmp;
a newly added unlisted fixture fails the count. The count alone would not prove
path uniqueness, but the current explicit paths are unique and each ran.

Fixtures cover renderer bytes; screenshots cover composed end and DM-1 mid
frames; serial assertions cover ordering, witnesses, exact pass text and
control outcomes; feature builds cover off-combinations. Protocol failure
codes and capability rights are code-audit properties checked directly in
item 1.

## 7. Dead scaffolding and discriminators — PASS

No scenario discriminator, witness line, negative control or control scenario
was removed. New kernel base features deliberately carry no SCENARIO or pass
string; each concrete boot feature does. The old not-ladders are obsolete, not
weakened. The xtask table replaces the old name lists and is held to
TEST_SCENARIOS; the current rows are unique. BaseScene::has_home was removed
after its last use; exact-symbol inspection found no caller. I found no other
dead scaffolding introduced.

## Verification

- RAG healthy (ollama=true, qdrant=true); semantic discovery preceded exact
  symbol inspection.
- cargo test -p neo-atrium: PASS — 48 passed, 2 ignored.
- cargo test -p xtask: PASS — 32 passed.
- cargo clippy -p xtask --all-targets -- -D warnings: PASS.
- Twelve-set neo-init clippy matrix listed in item 2: PASS.
- Fixture sweep: PASS — 14/14 byte-exact.
- One-byte fixture mutation: detected; exit 1; mutation restored.
- Optional QEMU scenarios: not run because LIMINE_DIR is unset.
