<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: inspect before run — what Arthur shows before it acts

**Date:** 2026-11-18. **Author:** the PM, asked by J.c. **Status:** design for review, no code.
**Read at:** `origin/main` `84f524e0`.
**Prompted by:** bashkit (`everruns/bashkit`, MIT, commit `7200e277`, 2026-11-18), a Rust bash
interpreter that runs commands in-process. Its `analyze()` reports what a script would run and write before
running anything, so the host can decide whether to ask the user. This note asks what that idea adds to
Arthur, given what Arthur already has. We borrow the pattern only. No bashkit code is copied, and none is
proposed.

## 1. The short answer

Arthur already does most of this, and does it the right way round:

- **The model emits a typed action, not shell text.** Arthur's decoder is constrained to a fixed set of
  tool calls (`agentic_action_logic.rs`), parsed into `AgenticAction`. No production in the grammar runs a
  command line, and none changes the posture (ADR-069).
- **Each action has an effect tier, set by code** (`agentic_effect_tier`, `agentic_action_logic.rs:515`):
  - R: read only;
  - T: recoverable from the trash;
  - L: irreversible on this machine (kill, spawn, load a model);
  - X: sends something off the machine;
  - U: refused.
- **The L/X preview is derived from the parsed action, not written by the model**
  (`push_agentic_a3_preview`, `conversation_logic.rs:1346`). It shows the tier, a digest, the action and its
  arguments, and the phrase to type.
- **The capability check decides, whatever the posture.** `execute_action()` refuses an ungranted action in
  every posture (Article 29.3, ADR-069's layer 2).
- **Paths, names and hosts are restricted to ASCII letters, digits and a few separators**
  (`require_valid_path` `:1284`, `require_valid_name` `:1328`, `require_valid_host` `:1342`). So a path
  can't hide a look-alike or reversed character in a preview.

So "inspect before run" is not a new subsystem for Arthur. It is five gaps around the edges of the one we
have (§2), plus one rule for the shell (§3).

## 2. The gaps

### G1. Free text shown to the user can carry invisible Unicode (fix first)

`require_valid_content` (`:1300`), `require_valid_phrase` (`:1314`) and `require_valid_question` (`:1549`)
refuse control bytes below `0x20`, `"`, `\` and `0x7f`. **They allow every byte from `0x80` up.** So the
written-file content, the dry-run plan, the confirm phrase and the clarify question can all carry:

- bidirectional overrides (U+202A–U+202E, U+2066–U+2069), which make the text display in a different order
  from its bytes;
- zero-width and other format characters (Unicode category Cf, such as U+200B–U+200F and U+2060), which
  are invisible.

The risk is the gap between what the user reads and what is stored or done. The worst case is the typed
confirm: if the phrase holds an invisible character, the user can't type what they see, or a phrase can be
made to look like a different action.

Whether the constrained decoder can actually emit these bytes depends on its vocabulary masks, which this
note doesn't check. **The validator is the place to make it impossible either way.**

**Fix:**
- Make the **confirm phrase ASCII-only**, printable `0x20`–`0x7e`. The user must be able to type exactly
  what they see.
- In **plan, question and content**, refuse the bidi and Cf ranges, or show them escaped (`\u{202E}`) in
  anything displayed.
- Add a KAT for each range, including one that passes the bytes through the real UTF-8 path.

This is bashkit's Unicode threat class (`TM-UNI`, 94 references in its threat model) applied to our preview.

### G2. The dry-run plan is the model's own words, shown like a system message

`AgenticAction::DryRun { plan }` prints `arthur: dry-run: <plan>` (`conversation_logic.rs:704-707`). The
plan is model-written text, not derived from anything. Everything else the user is shown before an
irreversible action is derived by code. This line reads the same way but isn't.

**Fix**, either of:
- **(a)** Relabel it as Arthur's own words (for example `arthur says (plan):`), and never put it next to a
  confirm phrase.
- **(b)** Drop the free-text plan, and have a dry-run list the derived effects of the staged action
  (tier, target, recovery), as the L/X preview already does.

**Recommendation: (a) now and (b) later**, once multi-step plans exist as data (G4).

### G3. The preview doesn't say whether the action would be allowed

An L/X action is staged and asks for a typed phrase before the capability check runs. If the grant is
missing, the user types the phrase and is then told "Denied". Layer 2 does its job, but the user was asked
to approve something that could never happen. Worse, they learn to type phrases without reading them.

**Fix:** a pure `would_permit(action, grants) -> Result<(), RequiredCapability>`, built from the same
`require` calls `execute_action_with` uses (`libneo/src/action.rs`). The preview then either:
- shows "needs: <resource> <rights>, not held" with no phrase to type; or
- shows the phrase as today.

The pre-check is **advisory only**. Execution still runs the real check, because grants can change between
the preview and the confirm. A KAT pins that the two functions agree on every action kind.

### G4. Multi-step tasks are confirmed one step at a time, with no running list

A multi-step session (`AgenticMultiStepSession`, `conversation_logic.rs:299`) confirms each L/X step as it
comes. The model picks each next step from the last result, so the whole plan isn't known up front, and a
bashkit-style analysis of the full script doesn't apply.

**What does apply:** each L/X preview inside a session also shows what the session has already done (the
`completed` entries, with their tiers and undo state). The user confirming step 3 then sees steps 1 and 2.
This is cheap, and it is the honest form of "show the whole effect".

A real up-front plan (the model proposing a list of actions as data, analyzed before any run) is a larger
design question. It is left open in §5.

### G5. Limits on work per request

bashkit bounds commands, loop iterations, function depth, output size, filesystem size, and how much work
the parser may do on one input ("parser fuel"). Arthur has field caps (128 bytes) and a reply cap
(`MAX_REPLY_BYTES`), and steps run one at a time. **Nothing to add today.** If `neo>` or Arthur ever gains
loops or scripts, take the whole set, parser fuel included, in the same build.

## 3. A rule for `neo>`: every command states its effects

`neo>` is deliberately not a POSIX shell (`docs/THINK-2026-07-08-poweruser-neo-shell-duality-design.md`,
S6). Today each line parses to one `ParsedCommand` (`userland/neo-shell/src/shell_logic.rs`), with no
pipes, no redirection and no scripts. So a line's effects are one-to-one with its parse.

The power-user design puts Arthur and `neo>` in one transcript, and Arthur will suggest `neo>` lines. Two
rules keep inspection cheap as `neo>` grows:

1. **No grammar production without an effect entry.** Each `ParsedCommand` variant comes with a pure,
   host-tested description of what it reads, writes, spawns or sends: its tier and targets, in the R/T/L/X
   terms Arthur uses. This is the same shape as "there is no posture production". A pipe, redirect or loop
   added later must add its entry in the same build, or the build is mis-specified.
2. **An Arthur-suggested line is shown through that description before it runs.** It goes through the same
   tier rules as a typed action: an L/X line needs the typed confirm, and a U line is refused. It still runs
   only with the shell's own capabilities.

This is bashkit's `analyze()` applied to our shell. Because our grammar is small and capability-scoped,
the description can be exact, where bashkit's has an "opaque" escape for constructs it can't see through.

## 4. Proposed rungs

| Rung | What | Touches | Gate | Kill-switch | Review |
|---|---|---|---|---|---|
| I1 | G1 (Unicode in shown text) and G2(a) (label the plan) | `neo-arthur` logic only | Host KATs plus the existing agentic QEMU scenarios' needles | The validators are a fix to an existing defect, so no switch; G2(a) changes one reply prefix, and the needles are updated in the same PR | Muse (security of Claude-authored code), plus a GPT-6-Luna code read |
| I2 | G3 (the advisory grant pre-check in the preview) | `libneo` `action.rs` (a pure function) and `neo-arthur` | Host KAT: `would_permit` agrees with `execute_action_with` on every kind, including deny paths; a QEMU needle for "not held, no phrase" | A feature; off = today's preview, byte for byte | GPT-6-Luna |
| I3 | G4 (completed steps shown in each L/X preview) | `neo-arthur` | Host KATs plus a multi-step QEMU needle | The same feature as I2 | GPT-6-Luna |
| I4 | §3 rule 1 (effect entries for `neo>`) | `neo-shell` `shell_logic.rs` | Host KAT: every `ParsedCommand` variant has an entry, checked exhaustively by a `match` with no wildcard | None needed (description only, no behaviour change) | GPT-5.6-Luna |
| Later | §3 rule 2 (Arthur suggests `neo>` lines), G2(b), G5 | — | — | — | — |

None of these touches the kernel or the SMP core, and each is small. I1 is the one with a security
finding behind it, so it goes first.

## 5. Open questions

1. **Should Arthur propose whole plans as data?** That is the only case where a bashkit-style analysis of
   the full set of effects applies before any step runs. It changes the multi-step design and the
   constrained grammar, so it needs its own THINK.
2. **Does the constrained decoder's vocabulary already exclude non-ASCII bytes in these fields?** If it
   does, G1 is defence in depth, not a live hole. Either way the validator is the right enforcement point,
   because it doesn't depend on the vocabulary.
3. **The coreutils lane (L1d).** bashkit ships a crate, `bashkit-coreutils-port`, that runs coreutils
   in-process with no fork or exec. That's the same constraint we have. It may be worth a row in L1d's
   survey; this note doesn't evaluate it.

## 6. Prior art

| Project | License | Last commit (checked) | What it does | What we take |
|---|---|---|---|---|
| bashkit (`everruns/bashkit`) | MIT | `7200e277`, 2026-11-18 | An in-process bash in Rust: a virtual filesystem, built-ins in place of programs, `analyze()` before run, resource limits, a threat model of ~2,550 lines with IDs cited in code | **Pattern only:** analysis before run (§3), the Unicode and injection threat classes (G1), the limit set (G5). No code |

Our own prior art is the larger part: ADR-069 (posture and tiers), the A3 design
(`Plans/THINK-2026-07-02-arthur-agentic-a3-design.md`), OT-26 D1 (constrained decoding) and Article 29.3.

## 7. What would prove this wrong

- **G1:** a KAT that feeds U+202E and U+200B through the real decode-and-parse path, and finds the parser
  already refuses them. Then G1 is already closed, and I1 shrinks to G2(a).
- **G3:** an action kind where `would_permit` and `execute_action_with` disagree. Then the pre-check can't
  be a shared pure function, and the preview must not claim "allowed".
- **§3:** a `neo>` command whose effects depend on run-time state the parser can't see (a target chosen by
  another command's output). Then rule 1 needs bashkit's "opaque" outcome, and an opaque line is treated as
  tier U for Arthur-suggested lines.
