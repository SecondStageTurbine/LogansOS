<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: The Atrium G0 buildable spec - single-Workfield prototype on the v1 framebuffer

**Date:** 2026-07-10
**Author:** Chad (Codex CLI)
**Source:** `Plans/DISPATCH-2026-07-10-atrium-g0-buildable-spec-think-chad.md`
**Branch:** `codex/atrium-g0-buildable-spec-think-chad`
**Status:** DESIGN THINK. Host-only. No code, no QEMU.

---

## 0. Decision in one breath

**G0 should build the "clean a folder" Workfield, not the "find/open a file" Workfield.**

Reason: "find/open" is the lower-risk read/nav demo, but it does not force the thing that makes The Atrium non-fake: a visible capability boundary plus a visible, real undo object. "Clean a folder" exercises the whole ratified paradigm in one bounded slice: Arthur turns an intent into a deterministic `neo>` Command Graph, candidate file cap-surfaces bloom from the transcript, missing FileRead/FileWrite authority becomes a visible OT-18 grant path, the action is recoverable through OT-19 session undo, and the Liveness Thread can honestly show tokens/action/cap-check/device-wait/undo/present. It is still small if G0 only supports one folder, one deterministic clean profile, recoverable trash/move only, and no permanent delete.

The buildable line is:

```text
Transcript turn:
  arthur> clean my Downloads folder

Workfield:
  one bounded folder-cleaning task bundle
  = transcript turn
  + exact neo> command graph
  + candidate cap-surfaces
  + cap grant/denial surface
  + one OT-19 Undo Shadow
  + honest Liveness Thread
```

G0 does **not** build a desktop, a compositor protocol, GPU text, multi-principal UI, or freeform windows. It is a single-principal, raw-framebuffer prototype that answers the first question: **does this feel obviously better than a terminal and un-fake compared with a desktop?**

---

## 1. ISC-1 - The one Workfield, fully specified

### 1.1 Workfield choice

G0 Workfield type: **Clean a folder**.

Recommended G0 utterance:

```text
arthur> clean my Downloads folder
```

Recommended deterministic equivalent:

```text
neo> clean-folder --profile g0-safe --preview /home/downloads
neo> clean-folder --profile g0-safe --apply --session-undo /home/downloads <candidate-id>...
neo> undo --session-last
```

If the G0 build wants to avoid adding a new shell command on the first commit, the same graph can be exposed as a generated record pipeline while the shell vocabulary catches up:

```text
neo> ls --records /home/downloads
neo> clean-candidates --profile g0-safe
neo> trash --session-undo <candidate-id>...
neo> undo --session-last
```

The rule is not the spelling. The rule is that Arthur's path and the `neo>` path share the same typed records and same cap checks. No hidden Arthur-only operation is allowed.

### 1.2 Workfield data model

G0 needs a concrete, serializable Workfield record. Suggested shape:

```text
Workfield {
  id: WorkfieldId,
  kind: CleanFolder,
  state: Dormant | Blooming | Previewing | WaitingForGrant | Applying | UndoAvailable | Complete | Dismissed,

  intent_spine: {
    transcript_turn_id,
    raw_user_text,
    source: ArthurTurn | NeoCommand | PointerOrKeys,
    created_at_ticks,
  },

  folder: {
    path,
    display_name,
    required_read: FileRead(path, LOOKUP | READDIR),
    required_write: FileWrite(path or trash_scope),
  },

  command_graph: CommandGraph,
  cap_surfaces: [CapSurface],
  candidates: [CleanCandidate],
  liveness: [LivenessSample],
  undo_shadow: Option<UndoShadow>,
  grant_surface: Option<GrantSurface>,
}
```

Candidate records are intentionally small:

```text
CleanCandidate {
  id,
  path,
  name,
  kind: File | Directory | Other,
  reason: TempName | CacheDir | DuplicateName | UserSelected,
  bytes: Option<u64>,
  selected: bool,
  required_action: TrashRecoverable,
}
```

G0 should avoid clever cleanup policy. The `g0-safe` profile only proposes items that are safe to explain on screen: temp-like names, cache folders, duplicates by exact name/hash if available, and user-selected files. If the policy cannot explain why a candidate is cleanable in one short phrase, it should not propose it.

### 1.3 Cap-surfaces

A cap-surface is a visible screen region backed by an authority requirement. In G0 the regions are local to the one Workfield, but they should be named the way G1 will later name compositor surfaces.

| Surface | Required cap | Purpose |
|---|---|---|
| Intent surface | none beyond current session | Shows the transcript turn that bloomed the Workfield. |
| Folder surface | FileRead folder lookup/readdir | Shows the folder path, candidate count, and read status. |
| Candidate surface | FileRead candidate metadata | One row/card per candidate, selectable. |
| Action surface | FileWrite trash scope + FileRead if undo snapshot needs it | Shows the exact recoverable mutation before apply. |
| Grant surface | Missing FileRead/FileWrite scope | Shows the OT-18 grant request in the butler register plus the deterministic `neo>` grant equivalent. |
| Undo Shadow | OT-19 session undo artifact | Shows what can be scrubbed back and when it expires. |
| Command Graph surface | none; mirrors actions | Shows the exact `neo>` graph Arthur is about to run or has run. |
| Liveness Thread | measured events only | Shows tokens/action/cap-check/device-wait/undo/present. |

G0 may draw these as boxes on a text grid. The important invariant is not the decoration; it is that the visible region's affordance is tied to the cap check that activation will perform.

### 1.4 Command Graph

The G0 Command Graph is a typed record pipeline, not prose.

```text
CommandGraph CleanFolderG0 {
  N0 Intent {
    text: "clean my Downloads folder",
    normalized_folder: "/home/downloads",
  }

  N1 ListDir {
    path: "/home/downloads",
    requires: FileRead("/home/downloads", LOOKUP | READDIR),
    emits: DirEntryRecord { path, name, kind, maybe_size, maybe_modified },
  }

  N2 CandidatePolicy {
    profile: "g0-safe",
    input: DirEntryRecord[],
    emits: CleanCandidate[],
  }

  N3 Preview {
    candidates: CleanCandidate[],
    selected: CandidateId[],
    reversible: true,
    emits: PreviewRecord { selected_count, selected_bytes, required_caps },
  }

  N4 ApplyRecoverableTrash {
    selected: CandidateId[],
    destination: "/home/downloads/.trash/session-<workfield-id>/",
    requires: FileWrite("/home/downloads") and any read cap OT-19 needs for snapshots,
    emits: MutationRecord { path, trash_path, undo_ref },
  }

  N5 ArmUndo {
    source: MutationRecord[],
    mechanism: OT19SessionUndo,
    emits: UndoRecord { action_id, bytes, expires: session_or_lru },
  }

  N6 PresentComplete {
    emits: PresentRecord { dirty_regions, frame_ticks },
  }
}
```

The visible `neo>` equivalent should be printed in the Workfield even when the user arrived through Arthur:

```text
neo> clean-folder --profile g0-safe --preview /home/downloads
neo> clean-folder --profile g0-safe --apply --session-undo /home/downloads 3 7 9
neo> undo --session-last
```

Power users can run the graph themselves. Beginners can ignore it. The graph remains present because power is depth, not a jailbreak.

### 1.5 Liveness Thread

The Liveness Thread is a measured channel. It may never be a decorative spinner.

G0 samples:

```text
LivenessSample {
  phase: IntentTokens | ActionCandidate | CapCheck | DeviceWait | UndoWrite | PresentComplete,
  status: Begin | Ok | Denied | Error,
  ticks_start,
  ticks_end,
  detail: short static text,
}
```

Required visible phases:

| Phase | What it proves |
|---|---|
| IntentTokens | Arthur actually ran a forward pass or selected a constrained action token. |
| ActionCandidate | The model or deterministic graph produced concrete candidate actions. |
| CapCheck | `execute_action` checked the required FileRead/FileWrite capability. |
| DeviceWait | Directory read/write/trash work is waiting on real IO, not UI theater. |
| UndoWrite | OT-19 created or refused the undo artifact. |
| PresentComplete | The Workfield updated the framebuffer after the state transition. |

The current v1 liveness indicator already brackets the real `sys_infer_run` path with "forward pass running" and "complete tokens=N". G0 generalizes that honesty to every visible phase.

### 1.6 Undo Shadow

The Undo Shadow is the visible OT-19 object for this Workfield.

It must show:

```text
UndoShadow {
  action_id,
  paths_affected,
  artifact_kind: Snapshot | TrashTombstone | PrewriteBin,
  bytes_held,
  expires: SessionEnd | LruLimit,
  state: Armed | Used | Expired | Unavailable,
}
```

Visual behavior:

- Before apply: a dim shadow behind the Action surface, labeled as "will be undoable" only if the undo preconditions are satisfied.
- During apply: Liveness Thread shows `undo-write begin` and `undo-write ok/denied/error`.
- After apply: the shadow becomes a selectable surface with the deterministic equivalent `neo> undo --session-last`.
- If undo is unavailable: the shadow is not drawn as armed; G0 must require confirmation or refuse the action according to the OT-19/A3 rules.

### 1.7 Bloom, update, dismiss

State machine:

```text
Dormant
  -> Blooming          when Arthur / neo> / input creates the Workfield
  -> WaitingForGrant   if N1 or N4 is denied
  -> Previewing        after candidate records exist
  -> Applying          after user holds/applies selected candidates
  -> UndoAvailable     after recoverable mutation succeeds
  -> Complete          after user accepts result or undo expires
  -> Dismissed         when user closes the Workfield
```

Dismissal does not delete the transcript turn or the action log. It only removes the Workfield surface from the raw framebuffer. Reopening from the transcript should reconstruct the read-only history plus the current undo state if the OT-19 artifact still exists.

---

## 2. ISC-2 - Rendering on the v1 framebuffer

### 2.1 Hard constraints from the current surface

The current framebuffer stack is deliberately simple:

- 32-bpp Limine linear framebuffer.
- Software primitives: clear, plot, rectangle fill, alpha RGBA blit, glyph draw, string draw.
- Current text is an 8x16 bitmap font.
- Current framebuffer console writes stdout bytes to the screen and scrolls by copying pixels.
- A single framebuffer lock serializes writes.
- No compositor, no double buffering, no GPU text, no retained scene graph.

That means G0 must be beautiful through layout, contrast, restraint, and truthful state changes. It cannot rely on GPU animation or zoom.

### 2.2 Compose with Prometheus's T0 text-engine seam

Prometheus's text-engine THINK defines the T0 seam as **crisp CPU/hinted type for G0 on the raw framebuffer**: FreeType/FTL in userland, an OFL font, anti-aliased glyph bitmaps, and blitting through the existing framebuffer alpha-blit path. This G0 spec should cite and consume that T0 layer, not re-derive the type stack.

Build rule:

- If T0 lands before the G0 build, G0 uses T0 for Workfield text.
- If T0 is not landed yet, G0 still builds using the current 8x16 bitmap font and cell geometry.
- The layout model must be identical in both cases: fixed regions, stable metrics, no text-overlap, no zoom dependency.

### 2.3 Minimal visual grammar

G0's raw-framebuffer grammar:

```text
Screen
  top craft bar       current mode, clock/ticks if available, "neo>" affordance
  left Intent Spine   transcript turns, active turn marker
  center Workfield    candidates, command graph, action preview
  right Thread        caps, liveness, undo shadow
  bottom rail         deterministic neo> equivalent and direct prompt
```

For a small framebuffer, collapse to vertical bands:

```text
Intent Spine
Workfield
Caps / Liveness / Undo
neo>
```

No nested cards. Each repeated candidate can be a compact cap-surface. The page sections are full-width bands or fixed columns, not floating dashboard panels.

### 2.4 Craft bar + Threshold resting state

The craft bar is not a menu bar clone. It is a quiet status strip:

```text
ATRIUM  Clean Folder  /home/downloads    caps: read ok, write missing    neo>
```

Use the current palette conservatively:

- Black/near-black background.
- Silver/neutral body text.
- Electric blue for active focus and owned capability.
- Muted blue for inactive structure.
- Alert magenta only for denied/missing authority or fatal state.
- Green/red only for functional pass/fail, not decoration.

The Threshold resting state is "intelligence as weather": when Arthur is idle, the Workfield should not display an avatar, face, mascot, or fake animation. The resting screen can show a dim Intent Spine, a quiet active-turn marker, and a dormant Liveness Thread. When real work happens, the measured liveness segments light up.

### 2.5 Cap Halo / material signals in software

G0 can draw Cap Halos with plain software primitives:

| Trust/grant state | Software material |
|---|---|
| Owned/granted | Solid silver border with electric-blue corner ticks. |
| Session/tethered grant | Dashed border plus a thin tether line to the grant surface. |
| Missing/denied | Alert-magenta left edge and "grant required" chip. |
| Untrusted/deferred | Muted stipple/dim border; not activatable. |
| Undo armed | Shadow band offset one row/4 px behind the action surface. |

For the current bitmap console, use ASCII-safe equivalents:

```text
[owned]        +----------------+
[session]      + - - - - - - - -+
[denied]       !----------------+
[undo armed]     ::::::::::::::
```

For T0, the same states can be drawn with rectangles, alpha blits, and hinted text. The state vocabulary must not change.

### 2.6 Motion within software limits

Permitted G0 motion:

- Row reveal as records arrive.
- Border state changes on cap-check result.
- Liveness segment changes only when a measured event begins or ends.
- Undo Shadow appears only after the undo artifact is armed.

Forbidden G0 motion:

- Fake spinners.
- Continuous ambient animation while idle.
- Physics that implies a compositor.
- Smooth zoom or transform effects reserved for G2.

---

## 3. ISC-3 - Input + intent-additive model

Intent is a peer of keys and pointer. It is additive, never a replacement.

### 3.1 Bloom paths

G0 has three bloom paths:

```text
Arthur turn:
  arthur> clean my Downloads folder
  -> constrained action / graph candidate
  -> Workfield blooms from transcript turn

neo> command:
  neo> clean-folder --profile g0-safe --preview /home/downloads
  -> same Command Graph
  -> same Workfield blooms from deterministic rail

Keys/pointer:
  user selects a folder/candidate/action surface
  -> same Workfield state machine
  -> same cap check before activation
```

For the first build, keys are enough because the shipped input path is fd 0 backed by serial/HID keyboard. Pointer input should be named as an additive path but not required for G0 unless a later build dispatch includes it.

### 3.2 Focus model

In G0, there is only one principal and no compositor, but focus should still behave like a capability check:

```text
FocusRequest {
  workfield_id,
  surface_id,
  principal: OwnerSession,
  required_cap,
}
```

If the surface requires no cap, focus moves. If the surface requires a missing cap, the Grant surface opens and focus moves there instead. If the surface is disabled because undo is unavailable or the action is too broad, the Workfield must explain that state; it may not silently ignore input.

Keyboard minimum:

| Key | Behavior |
|---|---|
| Tab / Shift-Tab | Move between surfaces. |
| Up/Down | Move candidate selection. |
| Space | Toggle candidate. |
| Enter | Activate focused surface. |
| Esc | Dismiss grant preview or Workfield. |
| `neo>` prompt | Always available at bottom rail. |

### 3.3 Transcript as Intent Spine

The transcript remains durable. The Workfield hangs off a transcript turn; it does not replace the transcript.

```text
Intent Spine record:
  turn id
  actor: arthur | neo | user
  text
  linked workfield ids
  outcome summary
```

When a Workfield blooms, the transcript should still show the plain-language Arthur response and the deterministic equivalent. The Workfield is the visible body attached to that turn.

---

## 4. ISC-4 - Cap model in G0

### 4.1 Single-principal, cap-gated regions

G0 is single-principal on the raw framebuffer. That does not make caps decorative.

Every activatable region has:

```text
CapSurface {
  id,
  rect,
  label,
  required_cap: Option<RequiredCapability>,
  grant_state: Granted | Missing | DeniedThisSession | Revoked | NotApplicable,
  focus_state,
}
```

Activation calls the same logical check as the action graph: if the action later requires `FileRead("/home/downloads")`, the visual surface that starts it must reflect that requirement before the user activates it.

### 4.2 Denial and grant path

A denial is visible and useful:

```text
Arthur: I have not been given the run of /home/downloads yet. A word from you and I will have it; shall I?

neo> grant fileread /home/downloads
neo> grant filewrite /home/downloads
```

The butler register belongs in the Grant surface. The deterministic grant equivalent belongs directly beneath it. "No" is remembered for the session to avoid nagging. "Always" records a durable grant in the OT-18 store, but the kernel cap is still the enforcement boundary.

### 4.3 Authority boundary

G0 should state this in the UI model and tests:

- `grants.json` is UX/audit persistence.
- The held kernel capability is the authority.
- `execute_action` checks the proposed action against the grant set before running syscalls.
- A fabricated grant record without a backed kernel cap still fails.

G0 must not add a visual-only grant state that can diverge from the action path. If the cap halo says granted, the action path must hold the needed cap. If the action path is denied, the halo must show missing/denied.

### 4.4 What G0 does not do

G0 does not implement:

- A compositor protocol.
- `SurfaceCap` / `BufferCap` / `InputFocusCap` / `PresentFenceCap` as cross-principal kernel/userland objects.
- Multi-principal isolation.
- Client-allocated buffers.
- Capture caps.
- Freeform windows.

G1 owns the cap-native compositor. OT-22 owns the multi-principal frontier. G0 only names the seam and keeps its local Workfield data model compatible with it.

---

## 5. ISC-5 - Gates G0 must pass

### 5.1 F1-F8 five-year-old checklist

| Gate | G0 pass condition |
|---|---|
| F1 request formable | "Clean my Downloads folder" works; no system terms required. |
| F2 narrated state | Workfield shows reading/candidates/grant/apply/undo/done. |
| F3 clarify, do not guess | Ambiguous folder or candidate policy asks before acting. |
| F4 one-sentence correction | "No, not that one" unselects/reverses without a procedure. |
| F5 plain errors | Missing cap and IO errors are butler/plain-language messages, not raw errno only. |
| F6 brief response | Arthur reports outcome, not effort. |
| F7 recognition cues | Candidates stay visible and referenceable. |
| F8 `neo>` bypass | The deterministic graph is visible and runnable. |

### 5.2 S1-S8 power-user checklist

| Gate | G0 pass condition |
|---|---|
| S1 composable | `clean-folder` graph consumes/emits typed records; no prose parsing. |
| S2 predictable | Same profile/folder/candidates produce the same preview. |
| S3 discoverable | `help clean-folder` and the Workfield show arguments and examples. |
| S4 deterministic | `neo>` path has no stochastic Arthur step. |
| S5 structured output | Preview and mutation output are records: candidates, caps, undo refs. |
| S6 capability-native | FileRead/FileWrite checks are the same checks Arthur uses. |
| S7 responsive | Prompt remains available while Workfield work waits on device/model. |
| S8 one substrate | Arthur and `neo>` see the same folder, grants, undo artifact, and action log. |

### 5.3 T1-T4 seam checklist

| Gate | G0 pass condition |
|---|---|
| T1 promotion discoverable | Arthur shows the exact `neo>` equivalent after creating the Workfield. |
| T2 transition fluid | User can move between Workfield and `neo>` without losing context. |
| T3 Arthur guides, not blocks | Dropping to `neo>` never requires Arthur approval. |
| T4 substrate unified | Arthur action and `neo>` command are two views over one graph. |

### 5.4 OT-41 GUI legibility gate

G0 inherits and extends OT-41:

- QEMU proves the flow: boot, greeting, Workfield bloom, candidate preview, denial/grant, apply, undo, `neo>` bypass, nonblank framebuffer, ordered serial proof.
- Hardware proves the thing is actually legible and beautiful on the PL's screen.

QEMU can prove correctness and nonblank output. It cannot prove the craft bar, cap halos, or Workfield text feel good on the physical framebuffer. The OptiPlex visual proof is part of the GUI gate, not a nice-to-have.

### 5.5 The one question

G0 answers "better than a terminal" if:

- The user can inspect candidates without mentally parsing a stream of lines.
- The exact command graph is visible without becoming mandatory.
- The grant and undo states are first-class objects.
- The user can recover without remembering a command.

G0 answers "un-fake vs a desktop" if:

- Cap halos are driven by real cap checks.
- The Liveness Thread is driven by measured work.
- Undo Shadow is driven by OT-19 artifacts.
- `neo>` runs the same substrate path.

If any of those are mocked, the prototype fails the premise.

---

## 6. ISC-6 - Buildable slice boundary + kill-switch

### 6.1 In G0

G0 includes:

- One Workfield type: Clean a folder.
- One active Workfield at a time.
- Single-principal owner session.
- Raw framebuffer rendering.
- Keyboard/Arthur/`neo>` bloom paths.
- Candidate preview.
- FileRead/FileWrite cap display and OT-18 grant path.
- Recoverable trash/move only.
- One OT-19 Undo Shadow.
- Honest Liveness Thread.
- `neo>` deterministic equivalent.

### 6.2 Deferred

Deferred to G1:

- Software cap-native compositor.
- Real `SurfaceCap` / `BufferCap` / `InputFocusCap` / `PresentFenceCap`.
- Multi-surface composition and damage tracking.

Deferred to G2:

- GPU vector text.
- Zoomable transcript/body.
- GPU-driven motion.

Deferred to OT-22:

- Multi-principal surface isolation.
- Untrusted app surfaces.
- Cross-principal input routing.

Deferred to later GUI/voice phases:

- Voice as a required bloom path.
- Freeform windows or instrument-mode windows.
- Wayland-compat island, if ever ratified.
- Full localization/shaping beyond the text-engine T0/T1/T2 sequence.

### 6.3 Kill-switch equality

G0 must be additive over the shipped v1 console.

Kill switch:

```text
atrium_g0=off
```

Required equality:

```text
G0 off == Janus v1 text console (`4858ac6`) bit-for-bit:
  boot -> ready -> Arthur greeting -> scrolling transcript
  -> honest thinking liveness around sys_infer_run
  -> neo> remains available
  -> no Workfield renderer, no altered transcript contract
```

Dismissed Workfield is not the kill switch; it is a user action inside G0. The kill switch disables the entire Workfield layer and leaves the prior v1 console surface intact.

---

## 7. ISC-7 - G0 build decomposition + honest forks

### 7.1 Eventual G0 build dispatch

Recommended build slices:

1. **Host model slice:** Workfield structs, Command Graph records, candidate policy, cap-surface states, Liveness Thread records, Undo Shadow records. Host tests only. No framebuffer.
2. **Text-frame renderer slice:** Render a Workfield frame into a fixed cell buffer. Verify no overlap, stable dimensions, cap halo states, and small-frame collapse. Host tests only.
3. **`neo>` graph slice:** Add or expose the deterministic `clean-folder` graph so the Workfield never depends on Arthur-only behavior. Host tests for typed records.
4. **Arthur bloom slice:** Hook the one Arthur intent to the same graph. The transcript prints the `neo>` equivalent.
5. **Cap/grant slice:** Wire denied FileRead/FileWrite to the OT-18 grant surface. Prove "No" anti-nag and "Always" persistence semantics without claiming `grants.json` is authority.
6. **Undo slice:** Wire recoverable apply to OT-19 session undo. Prove Undo Shadow only arms when the artifact exists.
7. **Framebuffer integration slice:** Draw the Workfield on the current raw framebuffer path or T0 text path. This is kernel-touching if it changes framebuffer/syscall behavior.
8. **Scenario gate slice:** Add `atrium_g0_clean_folder` scenario: boot -> greeting -> bloom -> preview -> cap denial/grant -> apply -> undo -> `neo>` bypass -> init continues.

If slice 7 touches kernel framebuffer routing, the build is kernel-touching: run the full close gate and the 4-shard QEMU `workflow_dispatch`. The OT-41 physical legibility proof is the GUI standing gate.

### 7.2 Smallest first commit

Smallest useful first commit for the future build:

```text
Host-only Workfield model + renderer snapshot:
  CleanFolder Workfield data model
  Command Graph records
  cap-surface states
  liveness samples
  undo shadow state
  text-cell renderer output fixture
```

This commit proves the shape before touching the kernel or burning runner time.

### 7.3 Honest forks

| Fork | Recommendation | Reason |
|---|---|---|
| Which Workfield | Clean a folder | It proves caps + undo + liveness in one bounded task. |
| How much visual grammar in software | Cell-first grammar with T0 optional | Works today on bitmap console; upgrades cleanly to CPU-hinted type. |
| Intent-bloom interaction | Transcript-anchored always | Preserves v1 transcript as Intent Spine and keeps `neo>` equal. |
| Candidate policy | Deterministic `g0-safe` only | Avoids fake AI cleanup and keeps preview explainable. |
| Apply behavior | Recoverable trash/move only | Gives OT-19 a real Undo Shadow without permanent destructive risk. |
| Pointer in G0 | Name the seam, do not require it | Keyboard/HID exists now; pointer focus belongs naturally with G1 input caps. |
| T0 text dependency | Consume if landed, fallback if not | Compose with Prometheus without blocking G0 on another lane. |

---

## 8. Source seams read

Primary design sources:

- `docs/THINK-2026-07-08-gui-vision-SYNTHESIS.md`
- `docs/OPEN-THREADS.md` rows OT-41, OT-45, OT-52, OT-18, OT-19, OT-22, OT-26, OT-46
- `docs/THINK-2026-07-07-usability-five-year-old-design.md`
- `docs/THINK-2026-07-08-poweruser-neo-shell-duality-design.md`
- `docs/THINK-2026-07-10-atrium-text-engine-research.md` (T0 text-engine seam)

Implementation seams read:

- `kernel/src/drivers/fb/mod.rs`
- `kernel/src/drivers/fb/console.rs`
- `kernel/src/drivers/fb/console_logic.rs`
- `kernel/src/arch/x86_64/syscall.rs`
- `kernel/src/drivers/usb/hid_keyboard.rs`
- `kernel/src/test_harness.rs`
- `userland/neo-shell/src/shell_logic.rs`
- `userland/neo-shell/src/main.rs`
- `userland/neo-arthur/src/agentic_action_logic.rs`
- `userland/neo-arthur/src/main.rs`
- `userland/libneo/src/action.rs`
- `userland/libneo/src/grant_store.rs`

RAG health was checked first and returned healthy (`ollama=true`, `qdrant=true`). The RAG was used only for discovery; exact seams above were read directly.

---

## 9. Close

Docs-only lane. No code and no QEMU. No CI run exists for docs-only path filters. The local close-gate host set should be run for familiarity per dispatch; if it times out on the docs branch, record that as a docs-branch skip note rather than inventing CI evidence.

RETURN should include:

- THINK path: `docs/THINK-2026-07-10-atrium-g0-buildable-spec.md`
- Chosen Workfield: Clean a folder
- Raw-framebuffer render plan with Prometheus T0 seam cited
- Input model: intent additive, transcript anchored
- Cap model: single-principal, cap-surface == cap-gated region, focus == cap check
- Gates: F1-F8, S1-S8/T1-T4, OT-41 hardware legibility
- Boundary: G0 vs G1/G2/OT-22
- Kill-switch: G0 off == v1 console bit-for-bit
- Build decomposition and honest forks
