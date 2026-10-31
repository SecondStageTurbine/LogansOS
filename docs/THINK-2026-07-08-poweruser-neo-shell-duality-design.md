<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: The power-user surface + the layer transition — neo> as first-class depth (the other half of the duality)

**Date:** 2026-07-08
**Author:** Prometheus (research / design lane)
**Source:** Plans/DISPATCH-2026-07-08-poweruser-neo-shell-duality-deepdive-prometheus.md
**Branch:** prometheus/poweruser-neo-shell-duality-deepdive
**Inputs:** Beginner half (THINK-2026-07-07-usability-five-year-old-design.md — 7 principles P1-P7, F1-F8 checklist); OS-substrate FINDINGS (RESEARCH-2026-07-08-beginner-poweruser-os-substrate-FINDINGS.md — P1-P7 verified); VISION-arthur-persona.md (butler register, 10 examples); Bible Article 1.5/1.6; OT-26 (v1 = conversation + neo>); fish shell design laws (fishshell.com/docs/current/design.html); PowerShell object pipeline (learn.microsoft.com); nushell structured data (nushell.sh/book); Unix philosophy (McIlroy, 1964); Findlater & McGrenere ISSE 2004 (self-morphing UI); Shneiderman multi-layer interfaces; Apple HIG disclosure widgets; ChromeOS developer mode anti-pattern; macOS one-substrate-two-views model

---

## 1. Problem statement — the power-user half of the duality

The five-year-old THINK proved the conversation surface is intuitive-by-construction (7 principles, F1-F8 checklist). The OS-substrate FINDINGS verified that systems serving both audiences do it with **one substrate wearing two views, where the user chooses when to go deeper and opening the power path never degrades the safe posture** (P1-P7).

What's unbuilt: the **power surface** — 
eo> as a first-class deterministic shell — and the **transition** between the transcript (layer 1) and 
eo> (layer 2), designed to the same buildable standard. The PL's frame: *"talk to Arthur, drop to neo> when you want precision."* This THINK makes "excellent for power users" as concrete and gate-able as the five-year-old test made "excellent for beginners."

**The thesis:** 
eo> must be a capability-native deterministic shell that is a joy for a power user — not a POSIX clone, not a fallback, not a place where you disarm the OS to get power. Depth = more granted capabilities, never fewer enforced invariants.

---

## 2. Q1 — What makes a power surface excellent (the neo> design standard)

### Shell prior art — what the great ones got right

**fish shell — discoverability as first principle:**
fish has five named design laws (fishshell.com/docs/current/design.html):
1. **Law of Orthogonality** — small set of orthogonal features; remove overlap
2. **Law of Responsiveness** — shell stays responsive even under filesystem contention; all syntax highlighting and autosuggestions are async I/O
3. **Configurability is the root of all evil** — "Every configuration option is a place where the program is too stupid to figure out what the user wants"
4. **Law of User Focus** — design the intuitive interface first, implementation details second
5. **Law of Discoverability** — "A program whose features are discoverable turns a new user into an expert in a shorter span of time"

Concrete mechanisms: tab completion with descriptions on every item, syntax highlighting that flags errors in red as you type, autosuggestions from history/completions/paths that expand visibly before execution.

**PowerShell — structured data pipeline:**
*"Most shells that only accept and return text, PowerShell accepts and returns .NET objects."* The pipeline carries fully-typed objects between cmdlets, not strings. This eliminates the "stringly-typed" problem where you must parse text output to extract structure. (learn.microsoft.com)

**nushell — typed I/O contracts:**
*"Contrary to the traditional Unix approach where commands communicate via plain text, Nushell commands communicate via data types: lists, records, or tables."* Every command declares explicit input/output types. Mismatched pipeline types produce errors, not silent garbage. (nushell.sh/book)

**Unix philosophy (McIlroy, 1964):**
Write programs that do one thing and do it well; write programs to work together; use text streams as the universal interface. The pipe is the composability primitive.

### Properties neo> must satisfy

**S1. Composability.** Commands are small, orthogonal, and chainable. 
eo> ls /home | grep notes | head 5 — each command does one thing, the pipeline composes them. The pipe is the universal interface between commands. This is McIlroy's core insight: composability through a single boring interface.

**S2. Predictability.** The command set is stable. Commands don't change behavior based on context, user history, or inferred skill level. 
eo> ls always lists directory contents with the same semantics. No self-morphing (Findlater & McGrenere: system-driven UI adaptation is measurably slower and disliked). Power users depend on muscle memory — a command that changes its arguments between sessions is hostile.

**S3. Discoverability.** help <command> shows what it does, its arguments, and examples. Tab completion enumerates available commands and paths. Syntax highlighting flags errors as you type. fish's Law of Discoverability: "features that are discoverable turn a new user into an expert faster." For neo>, this means the shell is self-documenting — a power user doesn't need external docs to learn what's available.

**S4. Determinism as a feature.** The neo> path is the reproducible, scriptable counterpart to Arthur's non-deterministic forward pass. Same input → same output, every time. No stochastic variation. This is what makes neo> the precision tool: when you need an exact result, you type the exact command. Arthur is for "find my notes"; neo> is for ls /home/notes | sort -r.

**S5. Structured over stringly.** Output is machine-parseable by default. 
eo> ls produces structured output (path, size, modified-time — not a wall of text). This enables piping to other commands without fragile regex parsing. PowerShell and nushell proved this: typed pipelines are more composable than string pipelines. For v1, tabular text output is sufficient (each column is a field); full object pipelines are a future enhancement.

**S6. Capability-native, not POSIX.** neo> is a from-principles capability-native shell. It is NOT a POSIX personality (that's OT-22/OT-44, decided elsewhere). Commands operate over the capability-secured substrate: 
eo> read /home/notes.txt checks the same cap-check as Arthur's ReadFile action. The shell is a capability tenant like Arthur — it can only do what its grants allow. Depth = more granted capabilities, never fewer enforced invariants.

**S7. Responsiveness.** The shell stays responsive. Completions and help are async — they don't block the prompt. fish's Law of Responsiveness: "the shell must stay responsive even under filesystem contention." For neo>, this means the prompt is always available even if a background action is running.

---

## 3. Q2 — The promotion affordance (the seam between transcript and neo>)

### Shneiderman + Apple disclosure model

Shneiderman's multi-layer interface design: novices start at a reduced-feature layer 1 and move up by their own choice — one system serving novice→expert without forking the product. Apple codifies this with disclosure widgets: essentials always visible, advanced functionality hidden behind a twistie that expands on request — **one dialog, two audiences, a stateful escape hatch, not two UIs** (Apple HIG).

**LogansOS mapping:** the transcript IS layer 1, neo> IS the higher layer. The load-bearing design is the *promotion affordance*: the layer-2 door must be visible-but-quiet from layer 1.

### The concrete patterns

**Discovery — Arthur names the deterministic equivalent (the disclosure button):**
After Arthur completes an action, it optionally names the neo> equivalent:
`
Arthur: Done — forty-three of them. The originals are recoverable this session.
        You can also do this yourself with: neo> rename --by-date /home/screenshots
`

This is the disclosure button. It's quiet (one line, after the result), visible (always there for actions that have a deterministic equivalent), and actionable (the user can copy-paste or type it). It reveals the power path without forcing it.

**First-boot — naming both layers:**
VISION Example 10 already does this:
`
Arthur: Good afternoon. I'm Arthur — I look after this machine for you.
        Tell me what you'd like, in your own words, and I'll see to it.
        If ever you'd rather do it yourself, "help" shows you the way.
`

The "help" command is the bridge. It should show both layers:
`
neo> help
Arthur commands (talk to me):
  "open my notes" — find and open files
  "what's running?" — list active processes
  "find files from yesterday" — search by time

Deterministic commands (type them yourself):
  ls <path> — list directory contents
  read <path> — read a file
  status — show system state
  grant <scope> <path> — grant Arthur authority

Switch between modes:
  arthur> — conversation mode (default)
  neo> — deterministic shell
`

**Fluid mid-session movement:**
The user can switch freely between rthur> and 
eo> in the same session. The transcript shows both:
`
arthur> find my notes about OptiPlex
Arthur: I found three files — optiplex-budget.xlsx, optiplex-notes.txt, and...
        You can also: neo> ls /home/notes | grep optiplex

neo> read /home/notes/optiplex-notes.txt
[content]

arthur> summarize that last file
Arthur: The notes cover three topics: budget allocation, timeline, and...
`

The transcript is the unified surface. Arthur turns and neo> turns are interleaved in the same conversation. The user doesn't "leave" Arthur to use neo> — they're using two tools in the same workspace.

**The butler as guide, not wall:**
Arthur never blocks the power path. VISION rule 9: "Always offer the exit." The butler is a convenience, not a captor. When the user drops to neo>, Arthur doesn't protest or re-engage. The shell is available, always, without friction.

---

## 4. Q3 — Power-is-not-a-jailbreak, operationalized (P5)

### The ChromeOS anti-pattern

ChromeOS "developer mode" disables verified boot and shows a full-screen warning on every boot: *"Developer Mode. Press any key to continue."* To access deeper capabilities, you must first disable core security protections. This trains power users that "more power = less security."

### The Windows registry anti-pattern (verified)

Microsoft's own docs: *"Serious problems might occur if you modify the registry incorrectly … These problems might require that you reinstall the operating system … Modify the registry at your own risk."* The power-user config path carries a reinstall-warning instead of being a safe first-class interface.

### LogansOS: depth = more granted capabilities, never fewer invariants

**The invariant:** 
eo>, the SDK, third-party bundles, and future dev tooling all run capability-bounded on the same kernel with **no "developer mode" that turns protections off.** There is no command that disables the cap-check. There is no sudo-equivalent that grants all capabilities. The shell is a tenant like Arthur — it can only do what its grants allow.

**How this shows up in neo> UX:**
`
neo> write /etc/config.txt "debug=true"
Denied: WriteFile on /etc requires FileWrite grant for /etc
        Grant it with: grant filewrite /etc
`

The denied action names the missing capability and offers the grant path. The user grants authority (OT-18) — they don't disarm the OS. This is OT-18 D4 (scope-level re-request) applied to the shell surface.

**Contrast with Arthur's denied:**
`
arthur> write to the system config
Arthur: I haven't been given the run of /etc yet. A word from you and I'll have it; shall I?
`

Same mechanism (cap-check denial), different voice (butler register vs plain shell output). Same substrate, two views.

**The grant is the power path:**
Power users get power by granting more capabilities to neo> — not by disabling protections. 
eo> grant filewrite /home gives the shell write access to /home. The cap-check still runs; it just has a broader grant set to match against. This is P5 operationalized: depth = more granted capabilities, never fewer enforced invariants.

---

## 5. Q4 — Never-adaptive on the power surface (P2)

### Findlater & McGrenere (ISSE 2004)

Controlled study (27 subjects, split menus): a static menu beat a system-driven adaptive menu on speed; a user-controlled adaptable menu beat the adaptive one; most users preferred the adaptable. Silent, system-driven UI adaptation measurably degrades performance and is disliked.

### Applied to neo>

**The command set is stable.** 
eo> ls has the same arguments and behavior every time. It doesn't change based on what the user has been doing, how often they use it, or what Arthur thinks they need.

**Arthur may suggest, never rearrange:**
`
Arthur: You use this daily — want me to set up an alias? "nn" for "neo> ls /home/notes | grep -i"?
User: yes
[alias is set — user-initiated, not system-initiated]
`

The suggestion is opt-in. The shell never silently changes its own behavior. fish's Law 3 applies: "Configurability is the root of all evil" — every config option is a place where the program is too stupid to figure out what the user wants. neo> ships with sensible defaults and lets the user customize explicitly.

**The line between helpful and hostile:**
- **Helpful:** Arthur suggests an alias; the user accepts or declines. Tab completion offers commands from history. Autosuggestions show likely completions as you type.
- **Hostile:** The shell changes command behavior based on inferred context. Commands silently add or remove arguments. The prompt changes its appearance based on "detected skill level."

Power users especially depend on predictability. A command that worked yesterday must work today with the same arguments and produce the same output.

---

## 6. Q5 — Two-views-one-substrate (P1, the UX contract)

### macOS: GUI + Terminal over one truth

Finder (GUI) and ls/cat/chmod/mv operate on the exact same VFS objects. Changes are immediately visible across both views. No divergence between GUI state and CLI state. The defaults system provides one typed config store underlying both GUI preference panels and CLI tools with no GUI exposure.

### LogansOS: transcript + neo> over one truth

**The UX contract:**
- Arthur saying *"I've turned that off, sir"* and 
eo> config get must show the same key.
- A file Arthur created is the same object 
eo> ls lists.
- A process Arthur spawned is the same process 
eo> status inspects.
- A grant Arthur requested is the same grant 
eo> grants shows.

**The mechanism (principle, not implementation):**
Both surfaces operate over the same capability-secured substrate. Arthur's actions go through xecute_action() with cap-check. neo> commands go through the same xecute_action() with the same cap-check. Both see the same NeoFS objects, the same process table, the same grant set. There is no "Arthur state" vs "shell state" — there is one system state, and both surfaces are renderings of it.

**What v1 can honor now:**
- File operations: Arthur's ReadFile/ListDir and neo>'s read/ls operate on the same NeoFS objects. ✓
- Process operations: Arthur's InspectProcess/SpawnProcess and neo>'s status/spawn operate on the same process table. ✓
- Grant operations: Arthur's on-miss re-request and neo>'s grant command modify the same grants.json. ✓

**What v1 can't fully honor yet (OT-50 tie-in, flagged):**
- System configuration: v1 has no unified config store yet. Arthur mediates grants.json/stakes.json as files; neo> reads them as files. They're the same files, but there's no typed config store with atomic read/write. This is P1 from the substrate FINDINGS — banked for PL ratification when the first real config surface builds.

---

## 7. ISC-1 — The neo> power-surface principles

**S1. Composability.** Commands are small, orthogonal, chainable via pipe. One thing, done well, works with everything else. (McIlroy, 1964.)

**S2. Predictability.** The command set is stable. Commands don't change behavior based on context, history, or inferred skill. Same input → same output, every time. (Findlater & McGrenere, ISSE 2004.)

**S3. Discoverability.** Self-documenting: help <command> shows arguments and examples. Tab completion enumerates commands and paths. Syntax highlighting flags errors as you type. (fish shell, Law of Discoverability.)

**S4. Determinism.** The neo> path is the reproducible, scriptable counterpart to Arthur's non-deterministic forward pass. No stochastic variation. Precision on demand.

**S5. Structured output.** Machine-parseable by default. Tabular text for v1 (each column is a field); object pipelines for future. No fragile regex parsing required. (PowerShell, nushell.)

**S6. Capability-native.** Not POSIX. Commands operate over the capability-secured substrate. Depth = more granted capabilities, never fewer enforced invariants. No jailbreak, no developer mode. (P5, ChromeOS lesson.)

**S7. Responsiveness.** The prompt is always available. Completions and help are async. The shell never blocks on a background action. (fish shell, Law of Responsiveness.)

---

## 8. ISC-2 — The promotion-affordance pattern (the seam)

### Discovery
Arthur names the deterministic equivalent after completing an action: *"You can also do this yourself with: neo> …"* Quiet (one line), visible (always there for actions with a deterministic equivalent), actionable (copy-paste or type).

### First-boot
VISION Example 10 names both layers. help shows both Arthur commands and deterministic commands, plus the mode-switch syntax (rthur> / 
eo>).

### Fluid movement
The transcript is the unified surface. Arthur turns and neo> turns are interleaved in the same session. The user doesn't "leave" Arthur — they use two tools in the same workspace.

### Butler as guide
VISION rule 9: "Always offer the exit." Arthur never blocks the power path. When denied, Arthur offers the grant path (butler register) or the shell shows the grant command (plain output). Same mechanism, different voice.

### The disclosure principle
When a surface has a simple and a full form, ship ONE surface with a disclosure state, never two parallel surfaces. (P3, Apple HIG.)

---

## 9. ISC-3 — The two-surface duality gate, operationalized

### The power-side checklist (S1-S8)

Every neo> command and shell behavior is evaluated against this checklist before shipping. A failure on any item is a design defect.

**S1. Is the command composable?** It does one thing, takes pipe input, produces structured output. It chains with other commands without intermediate files.

**S2. Is the command predictable?** Same arguments → same behavior, always. No context-dependent behavior changes. No self-morphing.

**S3. Is the command discoverable?** help <command> shows what it does, its arguments, and an example. Tab completion offers it. Syntax highlighting flags bad arguments as you type.

**S4. Is the command deterministic?** Same input → same output. No stochastic variation. Scriptable and reproducible.

**S5. Is the output structured?** Machine-parseable by default. Tabular text for v1. No wall of unstructured prose.

**S6. Does it respect the capability boundary?** The command goes through the same cap-check as Arthur's equivalent action. Denied actions name the missing capability and offer the grant path. No jailbreak, no developer mode.

**S7. Is the shell responsive?** The prompt is available even during background actions. Completions don't block input.

**S8. Does it honor two-views-one-substrate?** The neo> command operates on the same objects as Arthur's equivalent action. Changes visible from one surface are visible from the other. No divergence.

### The seam checklist (T1-T4)

Every layer-crossing interaction (transcript ↔ neo>) is evaluated against this checklist.

**T1. Is the promotion path discoverable?** Arthur names the deterministic equivalent after completing an action. The beginner can find neo> without being forced into it.

**T2. Is the transition fluid?** The user can switch between arthur> and neo> in the same session without losing context. The transcript shows both surfaces interleaved.

**T3. Does Arthur guide, not block?** When the user drops to neo>, Arthur doesn't protest or re-engage. VISION rule 9: always offer the exit.

**T4. Is the substrate unified?** Arthur's action and neo>'s command operate on the same objects with the same cap-check. No "Arthur state" vs "shell state."

### How to use this gate

The duality checklist (S1-S8 + T1-T4) gates power-facing and layer-crossing surfaces the way the five-year-old checklist (F1-F8) gates beginner-facing surfaces. Before A1 ships:

1. Run each neo> command against S1-S8.
2. Run the arthur> ↔ neo> transition against T1-T4.
3. Any failure is a design defect. Fix the design, not the user.
4. Document pass/fail in the A1 dispatch RETURN.

---

## 10. ISC-4 — Forks and gaps flagged

### Fork 1: Does v1 neo> need structured output from day one? (PL ratification point)

S5 recommends structured output (tabular text for v1, object pipelines for future). The question is: is tabular text enough for v1, or does neo> need full object pipelines (PowerShell/nushell style) from the start?

**Argument for tabular text (v1):** Simpler to implement. Power users can parse tables with standard tools (awk, cut). The composability requirement is met through pipes of structured text. Object pipelines require a runtime type system in the shell — significant complexity for v1.

**Argument for object pipelines (v1):** This is what makes modern shells excellent. Typed pipelines are more composable than string pipelines. If neo> is a from-principles shell, it should get this right from the start.

**Flag for PL:** Tabular text is the minimum viable structured output. Object pipelines are a quality-of-life enhancement that defers to a future neo> evolution. Recommend tabular text for v1; object pipelines as a post-v1 evolution.

### Fork 2: How much of P1's one-substrate contract can v1 honor?

The unified config store (P1 from the substrate FINDINGS) doesn't exist yet. v1 has grants.json and stakes.json as files — both surfaces read/write the same files, but there's no typed config store with atomic operations.

**Flag for PL:** v1 honors the contract for file/process/grant operations (same NeoFS objects, same process table, same grants.json). System configuration is a future surface (OT-50) — the contract is stated now so the future build satisfies it.

### Fork 3: What is neo>'s command vocabulary?

The dispatch doesn't ask me to design the command set. But the principles imply constraints: commands should map to ActionKinds (S6), be composable (S1), and produce structured output (S5). The command vocabulary is an A1 implementation detail, not a design fork.

**Flag:** A1 should author the neo> command vocabulary from the ActionKind enum — one source of truth, same as the Arthur action grammar. 
eo> read maps to ReadFile, 
eo> ls maps to ListDir, etc. This satisfies S6 (same cap-check) and S8 (same substrate).

---

## Sources

- fish shell design laws — https://fishshell.com/docs/current/design.html
- PowerShell object pipeline — https://learn.microsoft.com/en-us/powershell/scripting/overview
- nushell structured data — https://www.nushell.sh/book/nu_fundamentals.html
- Unix philosophy (McIlroy, 1964) — "The UNIX Time-Sharing System"
- Findlater & McGrenere, ISSE 2004 — "The Case against System-Driven Self-Morphing User Interfaces"
- Shneiderman, "Designing the User Interface" (4th ed., 2016) — multi-layer interfaces
- Apple HIG disclosure widgets — https://developer.apple.com/design/human-interface-guidelines
- Nielsen Norman Group, progressive disclosure — https://www.nngroup.com/articles/progressive-disclosure/
- ChromeOS developer mode anti-pattern — Wikipedia ChromeOS article
- Windows registry "at your own risk" — learn.microsoft.com registry documentation
- macOS one-substrate-two-views — macos-defaults.com

---

## Gate + close

Docs-only, no code changes. No CI run exists (paths-ignored per dispatch).
