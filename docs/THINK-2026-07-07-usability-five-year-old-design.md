<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# THINK: Usability that beats Windows — the five-year-old test, made buildable (Article 1.5)

**Date:** 2026-07-07
**Author:** Prometheus (research / design lane)
**Source:** Plans/DISPATCH-2026-07-07-usability-five-year-old-deepdive-prometheus.md
**Branch:** prometheus/usability-five-year-old-deepdive
**Inputs:** Bible Article 1.5/1.6 (docs/bible.md:21-28); VISION-arthur-persona.md (butler register, 10 examples); OT-26 THINK (THINK-2026-06-21-arthur-agentic-action-design.md — clarify-don't-guess D4, safety model D3); reliability FINDINGS (RESEARCH-2026-07-07-arthur-agentic-reliability-FINDINGS.md — constrained decoding, grammar-enabled clarify); Norman, *The Design of Everyday Things* (1988/2013); Nielsen, 10 Usability Heuristics (1994/2024, nngroup.com); Budiu, Recognition vs Recall (2024, nngroup.com); Murad et al., VUI Design Guidelines (2018, ACM MobileHCI); Myers et al., VUI Obstacle Patterns (2018, ACM CHI); Zhou, LLMs for Dialogue Survey (2024, TACL); Blackwell, "The Curb-Cut Effect" (2017, SSIR); NC State Center for Universal Design, Seven Principles (1997); Reid, "Curb-Cut Perils" (2022, SSRN)
**Scope:** Research + design THINK. No code, no QEMU.

---

## 1. Problem statement — from lens to gate

Article 1.5 declared the five-year-old test: **"a five-year-old can pick up a LogansOS-powered laptop and intuitively know how to use it."** The Bible says 1.5 is a "standing design lens" and that "when Arthur grows a real interface, this becomes a live design gate."

Arthur is growing a real interface. A1 ships the first agentic surface: NL → ListDir/ReadFile/InspectProcess → plain-language result. This THINK makes 1.5 buildable for A1 and every subsequent Arthur-facing surface. It turns the lens into principles, the principles into a checklist, and the checklist into a gate.

**The thesis to prove:** Conversation-first, with the right HCI principles, is MORE intuitive than the desktop metaphor for non-experts — not because it's simpler, but because it structurally eliminates the failure modes that make Windows hard for anyone who isn't already trained in its metaphors.

---

## 2. Q1 — Intuitive-by-construction: HCI principles specialized to conversation-first

### Norman's gulfs — what changes with NL

**Gulf of Execution** (gap between goal and action): **COLLAPSED by NL.** Norman's VCR example required 14 steps to record a show. In LogansOS: "save this video" → Arthur maps to the right action. The gulf of execution is the distance between what you say and what happens. For a five-year-old, that distance is zero — they already know how to form requests in natural language.

**Gulf of Evaluation** (gap between system state and user understanding): **HARDER in pure NL, solved by the transcript.** A five-year-old can't tell if Arthur is thinking, working, or done unless the system narrates its state. The solution is the persistent transcript — every Arthur interaction leaves a visible, scannable record. "Reading your notes..." → "Found three files from yesterday" → result. The transcript closes the gulf of evaluation by making the conversation itself the status display.

**Affordances:** **INVERTED in NL.** GUIs show affordances visually (buttons invite clicking). A blank prompt affords infinitely many inputs but signals zero constraints. This is the discoverability problem (Q3 below). The solution: Arthur's first-boot message (VISION-arthur-persona.md Example 10) establishes what's possible through conversation, not enumeration. "Tell me what you'd like, in your own words, and I'll see to it."

**Feedback:** **RICH but must be explicit.** Norman's rule: "For every operator action, there should be some system feedback" (Shneiderman's Golden Rule #3). In a CUI, feedback is the Arthur reply. The butler register enforces brevity and outcome-reporting (VISION rule 2: "Report the outcome, not the effort"). "Done — forty-three." is better feedback than a progress bar because it answers the question that matters: did it work?

**Constraints:** **SOFTER but more effective.** GUIs use hard constraints (disabled buttons). NL uses conversational constraints: Arthur clarifies before acting on ambiguous intent, confirms irreversible actions, and denies unauthorized ones with a plain-language explanation. VISION Example 4: "I haven't been given the run of the work drive yet. A word from you and I'll have it; shall I?" — this is a constraint expressed as an invitation.

### Nielsen's heuristics — conversation-first translation

| Heuristic | Conversation-First Application |
|-----------|------|
| **1. Visibility of System Status** | The transcript IS the status. Arthur narrates state transitions: "Reading...", "Found three files", "Done." Critical — this is what pure voice assistants failed to provide. |
| **2. Match Between System and Real World** | NL is the ultimate match. Users speak in their own language. "Trash that file," "delete it," "get rid of it" — all work because the LLM maps them to the same action. No system jargon required. |
| **3. User Control and Freedom** | "Stop," "undo that," "never mind" are always recognized. NL makes emergency exits natural language. OT-19 guarantees reversibility for destructive actions. |
| **4. Consistency and Standards** | The butler register (OT-46) provides personality consistency. The action grammar (OT-26 D1) provides behavioral consistency. If "open the file" works once, it works always. |
| **5. Error Prevention** | Grammar constraints prevent structural errors. Cap-check prevents authority errors. Clarify-don't-guess prevents intent errors. Three layers of prevention, each catching different failure classes. |
| **6. Recognition Rather Than Recall** | **THE CRITICAL INVERSION.** See below. |
| **7. Flexibility and Efficiency of Use** | NL inherently supports this. Novice: "find my tax returns." Expert: "search Documents for PDFs modified 2023 with tax in the name." Same system, different utterances, same result. |
| **8. Aesthetic and Minimalist Design** | In CUI, minimalism = brevity. VISION rule 1: "Fewer words than expected." Every extra word competes for attention. The transcript should be clean. |
| **9. Help Users Recover from Errors** | NL errors are different from GUI errors. Recovery is conversational: "I thought you said X — did you mean Y?" Plain-language error messages are native to CUI. |
| **10. Help and Documentation** | The system is self-explanatory through conversation. "What can you do?" yields a useful answer. VISION Example 10 (first boot) establishes this from the start. |

### Recognition-over-recall: how NL handles the critical inversion

Nielsen Heuristic #6 — "Recognition Rather Than Recall" — is the single hardest principle for pure NL interfaces. GUIs excel at recognition (visible menus, buttons, icons). Pure CUIs fail at recognition (the user must recall what to say).

**Budiu's analysis (2024):** Recognition is easier because "all those cues spread activation to related information in memory." A menu shows options; the user recognizes. A blank prompt offers no cues; the user must generate from scratch.

**How LogansOS solves this:**
1. **The transcript as recognition surface.** Previous turns are visible and referenceable. A five-year-old can see "Arthur found three files" and say "open the second one" — recognizing from context, not recalling from memory.
2. **Arthur's proactive suggestions.** After an action, Arthur can offer next steps: "Found three files from yesterday — the budget spreadsheet, the roofing quote, and the meeting notes. Which would you like to open?" This converts recall into recognition.
3. **The clarify branch as recognition aid.** When Arthur is uncertain, it presents candidates (VISION Example 6). "I've two candidates from yesterday, sir — the budget spreadsheet or the roofing quote." The user recognizes the right one.
4. **First-boot onboarding.** VISION Example 10 establishes what's possible through examples, not a manual. "Tell me what you'd like, in your own words" — this teaches the affordance of NL itself.

**The honest residual:** A first-time user who hasn't seen any Arthur interaction still faces a recall task. They must generate a request from nothing. This is why the first-boot message matters — it provides the initial recognition cue. After the first exchange, the transcript provides ongoing cues.

---

## 3. Q2 — Why Windows fails the five-year-old: anti-patterns our architecture removes

### The install ritual

**Windows:** Run installer → UAC prompt ("Do you want to allow this app to make changes?") → Accept EULA → Choose install path (default is fine?) → Next/Next/Next → "Restart now?" → if something fails mid-install, error code 0x80070005.

**HCI analysis:** This is a gulf of execution problem (Norman). The user's goal is "install this app." The system requires 6-12 subtasks spanning privilege escalation, legal review, and configuration choices. Each step introduces failure modes. For a five-year-old: every step is a wall. What is an EULA? What does "make changes to this device" mean?

**LogansOS removal:** OT-44's install-is-data model. The developer platform dispatch identifies capability-native distribution: third-party apps declare wanted caps → owner grants via OT-18 authority UX → kernel cap-check bounds them. No admin elevation during install. No EULA negotiation. The install is: Arthur downloads the app, reports what it wants to do, and asks for permission in plain language. VISION Example 4: "I haven't been given the run of the work drive yet. A word from you and I'll have it; shall I?" — this IS the install ritual, reduced to one sentence.

### Nested menu discovery (feature archaeology)

**Windows:** Find a feature requires drilling through Start → Settings → System → Display → Night light → Schedule. If you don't know the path, you can't find the feature. Nielsen Heuristic #6 violated: the user must recall the menu hierarchy.

**HCI analysis:** NN/g research shows task abandonment increases exponentially with menu nesting depth. Features exist but are invisible. This is discoverability failure (Norman).

**LogansOS removal:** No menus to discover — you ask. "Turn on night light mode" → Arthur maps to the right action. The gulf of execution is collapsed because NL bypasses the menu hierarchy entirely. A five-year-old doesn't need to know that display settings are three levels deep in Settings.

### Unattributed errors

**Windows:** "Operation could not be completed. Error code 0x80070005." Nielsen Heuristic #9 violated: error messages should be in plain language, precisely indicate the problem, and suggest a solution. Hex codes are system-oriented terms (violating Heuristic #2).

**HCI analysis:** Error codes shift the cognitive burden to the user. The user must Google the code, find a forum post, and translate it back into their context. This is the opposite of intuitive.

**LogansOS removal:** Arthur explains errors in plain language. VISION Example 8: "I can't raise it — it's not answering on the network. Powered on and on the same wire as us, it usually comes to hand; shall I keep trying, or leave it?" The error is explained, contextualized, and a next step is offered — all in the butler register.

### Modal dialog blindness

**Windows:** A modal window blocks interaction with everything behind it. The user can't see what's happening. Duplicate clicks create duplicate actions. Nielsen Heuristic #1 violated.

**LogansOS removal:** Conversation-first has no modals. Arthur narrates state transitions in the transcript. The user always sees what's happening and can interrupt at any time ("stop," "never mind").

### File/folder hierarchy confusion

**Windows:** Where did I save that file? Documents vs Downloads vs Desktop vs OneDrive. Research shows novice users think in recency ("the thing I was just working on") and association ("the file Bob sent me"), not spatial filing cabinets.

**LogansOS removal:** NL handles temporal and associative references naturally. "Find the file Bob sent me yesterday" → Arthur searches by metadata, not folder hierarchy. The five-year-old doesn't need to understand directories to find their files.

---

## 4. Q3 — Voice assistant post-mortem: why conversation-first plateaued and what Arthur does differently

### Why Alexa/Siri/Google failed as primary interfaces

These were conversation-first systems that plateaued at novelty status. They never replaced the desktop. Five reasons, sourced from HCI literature:

**1. Discoverability failure (Murad et al., 2018, ACM MobileHCI):**
> "With purely audio-based interaction, voice user interfaces tend to suffer from low discoverability: it is difficult for users to understand the scope of a system's capabilities."

Users didn't know what they could ask. The result: voice assistants were used for 3-4 commands (timer, weather, music) and abandoned for everything else. A blank speaker affords infinitely many inputs but signals zero constraints.

**Arthur's answer:** The persistent transcript provides a visible record of what Arthur can do. First-boot onboarding (VISION Example 10) establishes capability through conversation. After the first exchange, the transcript is a recognition surface — the user sees what worked before and can repeat or build on it.

**2. The "did it hear me?" feedback gap (Myers et al., 2018, ACM CHI):**
Users couldn't tell if the system was listening, processing, or had gone silent. Pure audio provides no persistent state. Nielsen Heuristic #1 violated.

**Arthur's answer:** The transcript closes this gap. Every interaction is visible. Arthur narrates state transitions ("Reading...", "Found three files", "Done."). The butler register enforces outcome-reporting (VISION rule 2). No ambiguity about whether Arthur heard or what it did.

**3. No real agency — they were search wrappers (Galitsky, 2019; Pearl, 2016):**
> "Most voice assistants today are capable of executing single commands very well but limited in their ability to manage dialogue beyond a narrow task."

Voice assistants couldn't chain operations, maintain context, or take autonomous action. They answered facts and played music. They were search engines with a microphone.

**Arthur's answer:** OT-26's agentic action layer. Arthur has real tool-use (13 ActionKinds), capability-bounded authority (Article 29.3), and multi-step reasoning (A3). Arthur doesn't answer questions — it takes actions on the user's behalf within its granted scope. This is the difference between a search wrapper and an agent.

**4. No recoverability:**
Voice commands are ephemeral. You can't undo "delete all my emails" by clicking undo — you have to say "undo," and the system may not support it. Nielsen Heuristic #3 violated.

**Arthur's answer:** OT-19 reversibility. Destructive actions produce undo artifacts. VISION Example 2: "Cleared — two hundred and eleven items set aside. They'll keep for the session if you find you wanted one back." Reversibility is baked into the architecture, not bolted on as a voice command.

**5. Privacy/cloud concerns (Fortune, 2019; Guardian, 2019):**
Always-on microphones, cloud processing, human contractors reviewing voice recordings. Trust destroyed.

**Arthur's answer:** On-device, capability-bounded, telemetry-free. Article 23.1 (reversed): "no surveillance persona." Voice input is push-to-talk / local-only (OT-46B). Arthur processes locally. No cloud dependency. No always-listening.

### The critical difference: form factor

Voice assistants lived on dumb speakers in kitchens. They had no screen, no keyboard, no context about what the user was working on. They were ambient appliances, not primary computing devices.

**Arthur is the primary interface on the primary device.** The laptop screen shows the transcript. The keyboard and microphone are input methods. Arthur has context about files, processes, and system state. It's not a cylinder in the kitchen — it's the OS.

---

## 5. Q4 — The clarify/feedback loop as the usability recovery mechanism

### Clarify-don't-guess is not just reliability — it's usability

OT-26 D4 designed the clarify branch as a reliability fallback: when the model is uncertain about intent, it asks rather than guesses. From a usability perspective, this is **the primary recovery mechanism** for a conversation-first system.

In a GUI, error recovery requires: (1) noticing the error, (2) finding the undo button, (3) clicking it. In a CUI, error recovery is conversational: Arthur says "I thought you meant X — did you mean Y?" and the user corrects in one sentence.

### The interaction pattern (butler register)

**VISION Example 6** is the canonical clarify pattern:
> **You:** open the thing from yesterday
> **Arthur:** I've two candidates from yesterday, sir — the budget spreadsheet or the roofing quote. Which did you have in mind?

This pattern satisfies three usability principles simultaneously:
1. **Recognition over recall** (Nielsen #6): Arthur presents candidates; the user recognizes the right one.
2. **Error prevention** (Nielsen #5): Arthur doesn't guess wrong and then require undo. It asks first.
3. **Plain-language recovery** (Nielsen #9): No error codes, no technical terms. The clarification is a natural conversation.

### Designing clarify into A1

For A1 (read/nav), the clarify pattern covers:
- **Ambiguous path:** "Did you mean /home/notes or /home/project/notes?"
- **Multiple matches:** "I found three files matching that description — which one?"
- **Missing parameter:** "Which file should I open?"
- **Uncertain intent:** "Do you want me to read the file or list the directory?"

The grammar enables clarify (reliability FINDINGS Q3: Clarify("question") is a valid action). The butler register gives it its voice. A1 should design the clarify prompt strings together with the action grammar — not as separate concerns.

### The honest residual: overconfident models

From the reliability FINDINGS: small models are overconfident and will emit a wrong-but-valid action rather than clarify. The grammar enables clarification; it doesn't guarantee usage. The backstop is the safety stack: cap-check denies, OT-19 reverses, and the user can always say "no, I meant..."

VISION Example 9 covers this recovery:
> **You:** no, I meant the *other* invoice
> **Arthur:** Ah — my mistake. I've put the first back as it was; the March invoice is open for you now.

This is OT-19's reversibility wearing a face. The butler owns the error lightly and fixes it in the same breath (VISION rule 7).

---

## 6. Q5 — The curb-cut effect: designing for the five-year-old makes it better for everyone

### The principle

**The curb-cut effect** (Blackwell, 2017, SSIR): "Solutions created for marginalized groups often benefit everyone." Named for sidewalk curb cuts originally built for wheelchair users but adopted by parents with strollers, travelers with luggage, delivery workers, and skateboarders.

Designing for the five-year-old is designing for:
- The non-technical adult who just wants to find a file
- The elderly user who doesn't understand folder hierarchies
- The stressed expert who wants to get something done fast without navigating menus
- The visually impaired user who benefits from NL interaction
- The motor-impaired user who can't precisely target UI elements

### Universal design principles applied (NC State, 1997)

| Principle | LogansOS Application |
|-----------|------|
| **Equitable Use** | NL serves non-literate users, visually impaired users, motor-impaired users. The same interface works for everyone. |
| **Flexibility in Use** | Type or speak. Brief or detailed. The same task, multiple interaction styles. 
eo> for experts, rthur> for everyone else. |
| **Simple and Intuitive** | NL requires zero training to form a sentence. The five-year-old test. |
| **Perceptible Information** | Multi-modal output: text transcript + optional audio. Arthur narrates state changes. |
| **Tolerance for Error** | Clarify, confirm, reverse. Three layers of error tolerance. VISION rule 7: "Own errors lightly and fix them in the same breath." |
| **Low Physical Effort** | No precise mouse targeting, no keyboard shortcuts to memorize. Speaking or typing a sentence requires minimal effort. |
| **Size and Space for Approach** | The interface works from any device, any position. Conversation-first is device-agnostic. |

### The expert doesn't lose anything

Reid's warning (2022): designing for accessibility must not mean designing *only* for the least capable user. The system must remain powerful for experts.

**LogansOS preserves expert power through 
eo>.** The deterministic shell is always available. An expert who knows mv /home/notes /tmp/archive can type it directly — no LLM forward pass, no latency. NL is a full forward pass (slow on CPU); 
eo> is instant. Both coexist: rthur> for "tell me what you want," 
eo> for "do this exactly."

This is Nielsen Heuristic #7 in practice: novices use NL; experts use the shell. The same system, different interfaces, no loss of capability.

**Google search is the precedent:** the search bar was designed for users who couldn't navigate directory structures. Power users adopted it because specific queries are faster than clicking through menus. NL is the same — simpler for novices, faster for experts who know what they want.

---

## 7. ISC-1 — The LogansOS usability principles

Every Arthur-facing surface must satisfy these seven principles. They are Norman/Nielsen specialized to conversation-first and grounded in our architecture:

**P1. Intent is the interface.** The user states what they want in their own words. The system translates to action. No menus, no commands to memorize, no metaphors to learn. (Norman: gulf of execution collapsed.)

**P2. State is narrated, not hidden.** Arthur reports its state continuously through the transcript. "Reading...", "Found three files", "Done." The user always knows what's happening. (Nielsen #1: visibility of system status.)

**P3. Recovery is conversational.** When something goes wrong — wrong intent, wrong file, wrong action — the user corrects in one sentence. Arthur clarifies before guessing, confirms before acting destructively, and reverses what it can. Error recovery is a conversation, not a procedure. (Nielsen #3/#5/#9: control, prevention, recovery.)

**P4. Recognition over recall.** The transcript provides ongoing cues. Arthur presents candidates when uncertain. First-boot onboarding establishes what's possible. The user recognizes options from context rather than recalling commands from memory. (Nielsen #6.)

**P5. Plain language always.** No error codes, no system jargon, no technical terms in user-facing output. Arthur explains errors and constraints in the butler register — plain language, dry, competent. (Nielsen #2: match between system and real world.)

**P6. Brevity is respect.** Fewer words than expected. Report the outcome, not the effort. Every extra word competes for attention. The transcript should be clean. (Nielsen #8: aesthetic and minimalist design; VISION rule 1.)

**P7. Power is preserved.** The deterministic 
eo> shell is always available. Experts bypass NL entirely. Conversation-first doesn't mean conversation-only. Simplicity for novices, speed for experts, no loss of capability. (Nielsen #7: flexibility and efficiency of use.)

---

## 8. ISC-2 — The five-year-old test, operationalized

### The design-review checklist

Every Arthur-facing surface (A1/A2/A3 actions, error messages, confirmations, the transcript UI) is evaluated against this checklist before shipping. A failure on any item is a design defect, not a user defect.

**F1. Can a five-year-old form the request?** The user can express their intent in a simple sentence without learning system terms. "Find my drawing," "open that file," "what's on my desktop" — these work without training. If the user must learn Arthur-specific vocabulary, the design fails.

**F2. Does Arthur narrate what it's doing?** The user can see (in the transcript) whether Arthur is thinking, working, or done. No silent processing. No ambiguity about system state.

**F3. When Arthur is uncertain, does it ask rather than guess?** Ambiguous intent triggers clarification, not a best-guess execution. The clarify branch is available in the grammar and used when confidence is low. VISION Example 6 is the pattern.

**F4. When Arthur is wrong, can the user correct it in one sentence?** "No, I meant the other one." Arthur reverses the wrong action and executes the right one. No multi-step undo procedure. VISION Example 9 is the pattern.

**F5. Are error messages in plain language?** No hex codes, no technical terms, no system jargon. The error message explains what went wrong and offers a next step. VISION Example 8 is the pattern.

**F6. Is the response brief?** Arthur reports the outcome, not the effort. "Done — forty-three." not "I have successfully renamed 43 files. Is there anything else I can help you with today?" VISION rule 1.

**F7. Does the transcript provide recognition cues?** Previous turns are visible and referenceable. The user can see what worked before and build on it. After the first exchange, the interface is recognition-based, not recall-based.

**F8. Is 
eo> available as the expert bypass?** The deterministic shell is always accessible. Power users aren't forced through NL. Conversation-first doesn't mean conversation-locked.

### How to use this gate

The five-year-old checklist gates Arthur-facing surfaces the way the fresh-reader test gates documentation and the byte-oracle gates format specs. Before A1 ships:

1. Run each A1 surface (read file, list dir, inspect process, clarify, error) against F1-F8.
2. Any failure is a design defect. Fix the design, not the user.
3. Document the pass/fail for each item in the A1 dispatch RETURN.

---

## 9. ISC-3 — The A1 UX recommendation

### How the read/nav agentic loop should feel

**First interaction (cold start):**
`
Arthur: Good afternoon. I'm Arthur — I look after this machine for you. Tell me what you'd like, in your own words, and I'll see to it. If ever you'd rather do it yourself, "help" shows you the way.
User: what files do I have?
Arthur: You have three files from today — budget.xlsx, roofing-quote.pdf, and meeting-notes.txt. Which would you like to open?
`

**Clarify pattern:**
`
User: open the thing from yesterday
Arthur: I've two candidates from yesterday, sir — the budget spreadsheet or the roofing quote. Which did you have in mind?
User: the quote
Arthur: [opens roofing-quote.pdf] Done — fourteen pages.
`

**Error recovery:**
`
User: open the budget
Arthur: [opens budget.xlsx]
User: no, I meant the meeting notes
Arthur: Ah — my mistake. The meeting notes are open for you now.
`

**Plain-language error:**
`
User: read the file on the work drive
Arthur: I haven't been given the run of the work drive yet. A word from you and I'll have it; shall I?
`

### Concrete guidance for A1

1. **Transcript is the UI.** Every interaction leaves a visible, scannable record. Design the transcript layout (speaker labels, timestamps, action indicators) as part of A1. This is the recognition surface (P4) and the status display (P2).

2. **Clarify strings are butler strings.** The clarify prompt is VISION Example 6. Don't write generic "I didn't understand, could you rephrase?" — write "I've two candidates — which did you have in mind?" Design clarify strings together with the persona, not bolted on later.

3. **Error messages are butler messages.** Denied actions use VISION Example 4 pattern ("I haven't been given the run of X yet"). Syscall errors use VISION Example 8 pattern ("I can't raise it — it's not answering"). No error codes. No technical terms.

4. **Brevity is enforced by the persona.** VISION rule 1: "Fewer words than expected." Arthur doesn't narrate effort. "Done — forty-three." is the template. A1 should enforce this in the response generation prompt.

5. **First-boot onboarding sets the affordance.** VISION Example 10 teaches the user what's possible through conversation. Don't enumerate capabilities — demonstrate one and invite the next.

---

## 10. ISC-4 — Gaps and forks flagged

### Fork 1: How much visual UI does v1 need? (PL ratification point)

The five-year-old test was conceived for a conversation-first interface, but pure conversation has limits:
- A five-year-old reads shape and color before words (Article 1.6).
- Visual recognition is faster than verbal recognition for spatial tasks ("which file?" is easier to answer with icons than with names).
- The transcript provides some visual context, but it's text-only.

**Question:** Does A1 need any visual surface beyond the transcript? A file list rendered as named items in the transcript works for read/nav. But as Arthur grows capabilities (file management, process monitoring), a visual component may be necessary for the five-year-old test to pass.

**Flag for PL:** This is a roadmap fork. A1 can ship with transcript-only. A2/A3 may need visual augmentation. The decision should be driven by usability testing against the five-year-old checklist, not speculation.

### Fork 2: Voice input for the five-year-old (OT-46B)

A child speaks before they type. OT-46B banks voice input (STT) as a Phase-N build. For the five-year-old test to be literal — not aspirational — voice input may be required.

**Flag for PL:** Voice input is a new model type (same substrate decision as OT-16/DeltaNet). It's deferred until the conversation-first text interface is proven. But the five-year-old test may require it to be fully meaningful.

### Gap 1: Transcript design

The transcript is identified as the critical recognition surface, status display, and error recovery mechanism. But its visual design is unspecified. A1 should include basic transcript layout (speaker labels, action indicators, scroll behavior) as part of the UX recommendation. This is not a fork — it's an A1 implementation detail.

### Gap 2: Discoverability for first-time users

A first-time user facing a blank Arthur prompt has no recognition cues. The first-boot message provides initial affordance, but it's a single message in a transcript that doesn't exist yet. Consider a brief onboarding sequence (3-4 examples of what Arthur can do) as part of A1's first interaction.

---

## Sources

- Norman, *The Design of Everyday Things* (1988/2013) — gulfs of execution/evaluation, affordances, discoverability
- Nielsen, 10 Usability Heuristics (1994/2024) — https://www.nngroup.com/articles/ten-usability-heuristics/
- Budiu, "Memory Recognition and Recall in User Interfaces" (2024) — https://www.nngroup.com/articles/recognition-and-recall/
- Murad et al., "Design guidelines for hands-free speech interaction" (2018, ACM MobileHCI) — VUI discoverability failure
- Myers et al., "Patterns for How Users Overcome Obstacles in VUIs" (2018, ACM CHI) — feedback gap, recovery patterns
- Galitsky, *Developing Enterprise Chatbots* (2019, Springer) — voice assistant limitations
- Pearl, *Designing Voice User Interfaces* (2016, O'Reilly) — single-command limitation
- Zhou, "Large Language Models for Dialogue: A Survey" (2024, TACL) — multi-turn recovery patterns
- Blackwell, "The Curb-Cut Effect" (2017, SSIR) — https://ssir.org/articles/entry/the_curb_cut_effect
- NC State Center for Universal Design, Seven Principles (1997) — universal design framework
- Reid, "The Curb-Cut Effect and the Perils of Accessibility without Disability" (2022, SSRN) — warning against over-simplification
- Fortune, "Apple, Google, and Amazon May Have Violated Your Privacy" (2019) — voice assistant privacy failures
- Guardian, "Amazon staff listen to customers' Alexa recordings" (2019) — trust destruction

---

## Gate + close

Docs-only, no code changes. No CI run exists (paths-ignored per dispatch).
