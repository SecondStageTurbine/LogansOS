<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RESEARCH FINDINGS: Zorin OS deep research — what earns the review, transposed to a conversation-first OS

**Date:** 2026-07-11
**Author:** Prometheus (research / design lane)
**Source:** `Plans/DISPATCH-2026-07-11-zorin-usability-research-prometheus.md`
**Branch:** `prometheus/zorin-usability-research`
**Method:** Primary sources read directly — the Jack Wallen ZDNet review (located and quoted exactly), Zorin's own help docs, zorin.com product pages, technical details page. 3-vote verification discipline applied to load-bearing claims (noted per claim). Sibling study: `RESEARCH-2026-07-08-beginner-poweruser-os-substrate-FINDINGS.md` (P1–P7, four lenses).

---

## The review we're mining

**Source:** Jack Wallen, "I've used Linux for 30 years — here's how I'd rank DistroWatch's top 10," *ZDNet*, July 9, 2026.
**URL:** https://www.zdnet.com/article/how-id-rank-distrowatch-top-10-as-linux-expert/

Wallen ranks Zorin OS **#1 out of DistroWatch's top 10** on usability. The exact paragraph (verbatim):

> I've been calling Zorin OS one of the most usable Linux distributions on the market for some time. Zorin OS is the ideal distro for anyone coming from MacOS and/or Windows. Zorin OS is exactly what an operating system should be, be it open-source or proprietary. One reason Zorin OS stands above the rest is the ability to easily choose (or change) the layout so the UI is perfectly familiar. Zero learning curve is always good for usability.
>
> Another helpful feature comes by way of open-source app suggestions. If you download and attempt to install a Windows application, Zorin OS will suggest an open-source equivalent.

**Wallen's criteria (extracted from his full ranking methodology):** Usability = how fast can someone who doesn't already know Linux start doing their work? The two features he names as differentiators are: (1) layout familiarity eliminates learning curve; (2) the app-suggestion system prevents dead ends when the user reaches for something foreign.

---

## ISC-1 — The Zorin mechanism inventory, firsthand

### (a) Zorin Appearance — the layout switcher

**What it is:** A GNOME Shell extension + theme pack shipped as the "Zorin Appearance" system settings tool. Verified against zorin.com/os and the Pro page.

**What it changes:**
- **Panel position and composition:** top panel (macOS dock style), bottom panel (Windows taskbar style), or left sidebar (Ubuntu/GNOME style).
- **Dock behavior:** centered vs left-aligned icons; autohide vs persistent.
- **Menu structure:** Windows-style Start menu (bottom-left, hierarchical), macOS-style app switcher (top-left), or traditional GNOME overview.
- **Window controls:** position of minimize/maximize/close buttons (left = macOS convention, right = Windows convention).
- **Theme assets:** icon sets and GTK themes tuned per layout to reinforce the visual resemblance.

**What it deliberately does NOT change (the uncanny-valley boundary):**
- The underlying desktop environment remains **GNOME Shell** — all layouts are cosmetic rearrangements of GNOME's shell extensions, themes, and dash-to-dock/dash-to-panel configurations. The window manager, compositor, file manager (Nautilus), and system settings remain GNOME.
- Application behavior is unchanged. A LibreOffice window doesn't act more "Windows-like" in Windows mode.
- Keyboard shortcuts remain GNOME defaults unless the user changes them manually.
- The underlying package management, update system, and configuration model are Ubuntu/GNOME regardless of layout.

**Free vs Pro (verified against zorin.com/os/pro):**

| Layout | Core (free) | Pro ($39 one-time) |
|---|---|---|
| Windows-like (icons) | ✓ | ✓ |
| Touch | ✓ | ✓ |
| Windows List-like | ✓ | ✓ |
| GNOME Shell-like | ✓ | ✓ |
| macOS-like | ✗ | ✓ |
| Windows 11-like | ✗ | ✓ |
| Windows Classic-like | ✗ | ✓ |
| ChromeOS-like | ✗ | ✓ |
| Compact panel | ✗ | ✓ |
| Ubuntu-like | ✗ | ✓ |
| elementary OS-like | ✗ | ✓ |
| Cinnamon-like | ✗ | ✓ |

**Cost to Zorin to maintain:** Each layout is a curated set of GNOME Shell extension configurations + theme assets. The maintenance burden is the GNOME extension compatibility surface — every GNOME Shell update can break custom layouts. Zorin's team must test and patch after each GNOME release cycle. This is a **theming/UX** cost, not a kernel or substrate cost.

**Review sentence earned:** *"One reason Zorin OS stands above the rest is the ability to easily choose (or change) the layout so the UI is perfectly familiar. Zero learning curve is always good for usability."* — The layout switcher IS the mechanism that earns this sentence. It makes the first screen look like home.

### (b) The Windows-app detection + open-source suggestion pipeline

**How it works (verified against help.zorin.com/docs/apps-games/windows-app-support/ and help.zorin.com/docs/apps-games/alternatives-to-windows-apps/):**

1. **Detection:** When the user double-clicks a `.exe` or `.msi` file in Nautilus, Zorin OS intercepts the file association. If this is the first Windows app attempt, it presents an "Install Windows App Support" button (which installs the Wine compatibility layer).

2. **The suggestion path:** The Windows App Support help page explicitly says: *"Before installing any Windows apps in Zorin OS, we strongly recommend that you check whether a native version of the app is available to install from the Software store or the developer's website. If the app doesn't have a native Linux version, we recommend that you try a native alternative to this app for the best experience."* It then links to the **Alternatives to Windows Apps** page.

3. **The equivalence database:** The "Alternatives to Windows Apps" page is a **curated, hand-maintained markdown document** categorized by function (Office, Graphics & Reading, Sound & Video, System Tools). It maps ~25 popular Windows apps to cross-platform alternatives. Each entry lists the Windows app name, then 3-6 alternatives with links, Open Source badges, and whether they're pre-installed (Core vs Pro).

4. **Fall-through:** If no equivalent is listed, the user proceeds to Wine. The Wine layer is optional ("Install Windows App Support layer separately" — a menu item under System Tools). Zorin's docs caveat: *"not all Windows apps may be fully compatible with Windows App Support in Zorin OS."*

**What this actually is:** A **two-tier redirect** — (1) "check for native first" guidance, then (2) a curated alternative list, then (3) Wine as the last resort. The "suggestion" Wallen praises is primarily the curated alternatives page, not an automated runtime detection system. It's a documentation artifact, not a live database query.

**Cost to maintain:** Manual curation. The page was last modified May 2026. Each new popular Windows app requires a human to add it. The pre-installed annotations require tracking which edition ships which package.

**Review sentence earned:** *"Another helpful feature comes by way of open-source app suggestions. If you download and attempt to install a Windows application, Zorin OS will suggest an open-source equivalent."* — Wallen experienced the .exe double-click → suggestion flow. The mechanism is the file-association intercept + curated alternatives page + Wine fall-through.

**3-vote verification on this claim:**
- **Vote 1 (mechanism):** The .exe intercept is documented in Zorin's own help — the "Install Windows App Support" dialog appears on first double-click of a .exe/.msi. ✓
- **Vote 2 (suggestion quality):** The alternatives page is hand-curated, ~25 entries, categorized by function. It is NOT an automated equivalence database. ✓
- **Vote 3 (Wallen's experience):** Wallen wrote this July 9, 2026. The alternatives page was last modified May 2026. The timing is consistent — he experienced the current system. ✓

### (c) The arrival experience — first-boot and onboarding

**What Zorin ships (verified against install guide, zorin.com/os, and the "Switch Your Organization" doc):**

- **Installer:** Standard Ubuntu ubiquity installer with Zorin branding. Dual-boot option ("Install Zorin OS alongside [your current OS]"). The installer is graphical, step-by-step, with large buttons and minimal jargon.
- **Post-install first boot:** No dedicated "Zorin Tour" wizard was found in the help docs. The arrival experience is: boot → GNOME desktop in the default Windows-like layout → pre-installed apps visible in the menu.
- **Pre-installed software posture:** Core ships with a curated set: Brave browser, LibreOffice, Evolution mail, Remmina RDP, Zorin Connect (phone integration), basic utilities. The Software center is available for more. Pro adds ~20 professional apps (GIMP, Krita, Inkscape, Blender, Kdenlive, Ardour, etc.).
- **"Switch Your Organization" doc:** Zorin publishes a 5-step migration guide for organizations moving from Windows — indicating they treat the migration path as a first-class concern.

**Cost to maintain:** The pre-installed package list per edition is tracked on the technical details page (downloadable .zip per edition). Each major release requires curating which packages ship in which edition.

### (d) The edition structure

**Verified against zorin.com/os/pro and zorin.com/os/details:**

| Edition | Price | Desktop | Key differentiator |
|---|---|---|---|
| **Core** | Free | GNOME Shell | 4 layouts, standard apps |
| **Pro** | $39 one-time | GNOME Shell | 12 layouts, pro creative suite, productivity tools, wallpapers, installation support |
| **Lite** | Free | XFCE | For older hardware (15-year-old PCs) |
| **Pro Lite** | $39 | XFCE | Pro apps on lightweight desktop |
| **Education** | Free | GNOME Shell | Pre-configured for schools |

**What Pro actually sells:** 8 additional layouts + ~20 professional/creative apps + productivity tools (Deskflow, Planify, Xournal++, Network Displays) + wallpapers + email installation support. The Pro creative suite is positioned as "alternatives to over $5,000 of professional software" (AutoCAD, Maya, Premiere Pro, etc.).

**3-vote verification:**
- **Vote 1:** Pro's layouts are the primary paid differentiator for usability — the macOS-like and Windows 11-like layouts are Pro-only. This directly gates the "zero learning curve" feature behind payment for some source OSes. ✓
- **Vote 2:** The app bundle is the secondary differentiator — professional creative tools that a power user would otherwise hunt for individually. ✓
- **Vote 3:** Lite/Pro Lite split the same axes along desktop weight (GNOME vs XFCE), confirming the edition matrix is: {Core, Pro} × {full, Lite} + Education. ✓

---

## ISC-2 — The migration psychology, separated from the mechanism

### Principle M1: Meet the user where they already are (vocabulary + muscle memory)

**What Zorin verified:** A Windows migrant knows where the Start button is (bottom-left). A macOS migrant knows where the menu bar is (top). Zorin's layout switcher places these elements where the user expects them. The first 30 seconds of interaction cost nothing because the muscle memory transfers.

**What it costs Zorin:** GNOME extension maintenance per layout. The uncanny-valley risk (see anti-lessons below).

**Where mimicry hits its ceiling:** The layout makes the *screen* look familiar, but the *behavior* doesn't fully match. A Windows-like panel doesn't make LibreOffice behave like Word. It doesn't make the file manager handle NTFS permissions. The appearance promises more than the substrate delivers. This is where the migrant's disappointment begins: "It looks like Windows, so why doesn't it work like Windows?"

### Principle M2: The first-five-minutes cost must be zero

**What Zorin verified:** The default layout (Windows-like) means the first thing the user sees is already familiar. No onboarding wizard is needed to explain the desktop metaphor because they already know it. The pre-installed apps cover the basics (browser, office, mail) so the user can start working immediately.

**The critical insight:** Zorin doesn't win with features — it wins with **not making the user learn anything new in the first five minutes.** The layout does the teaching by being already-known.

### Principle M3: Redirect rather than reject (the app-suggestion pattern)

**What Zorin verified:** When the user reaches for a Windows app (.exe), Zorin doesn't say "this won't work." It offers a path: native alternative → curated equivalent → Wine compatibility layer. The redirect chain has three steps before it hits a dead end.

**The psychological mechanism:** The user's intent ("I need to do X") is acknowledged even when their chosen tool (.exe) is foreign. The system translates the intent into a native path rather than rejecting the foreign format. This is **redirect-never-reject** in practice.

### Principle M4: Familiarity as a bridge vs familiarity as an identity

**What Zorin gets right:** The layout switcher is a *starting point*. A user who arrives via Windows-like layout can later switch to GNOME Shell-like or Ubuntu-like as they learn Linux. The familiarity fades naturally because the tool makes it easy to change.

**Where Zorin stumbles:** By making mimicry the headline feature, Zorin trains users to see it *as* a Windows/macOS clone. The Pro page sells layouts as "macOS-like," "Windows 11-like" — these are borrowed identities. The user who paid for Pro expects ongoing Windows/macOS parity in behavior, not just appearance. This is the mimicry-as-permanent-identity trap.

---

## ISC-3 — The transposition (the payoff)

### The redirect pattern, generalized: Familiarity translation layer as Arthur knowledge

**Zorin's mechanism:** User double-clicks .exe → file association intercept → curated alternatives page → Wine fall-through.
**The principle:** When the user reaches for something foreign, translate their intent into a native path. Never a dead end.

**LogansOS-native transposition:**

The user speaks in Windows/macOS idiom. Arthur understands the foreign vocabulary as *intent*, does the native thing, and gently names it:

| User says (foreign idiom) | Arthur hears (intent) | Arthur does (native) | Arthur says (butler register) |
|---|---|---|---|
| "Open Task Manager" | List running processes | `InspectProcess` action | "Here's what's running, sir — four processes." |
| "Where's the Control Panel?" | System configuration | Routes to config domain | "That's the system settings — what would you like to adjust?" |
| "Empty the Recycle Bin" | Permanently delete trashed files | OT-19 permanent-delete path | "Permanently removing the trashed items — twelve files. That one I can't undo, sir." |
| "Install this .exe" (drops file) | Install this application | OT-44 catalog lookup → suggest native equivalent | "I don't have that one natively, sir. There's [equivalent] in the catalog — it does the same thing. Shall I pull it?" |
| "Where are my Downloads?" | Recent acquired files | ListDir on Downloads path | "You've three items from today — the budget spreadsheet, the roofing quote, and the meeting notes." |
| "Minimize all windows" | Clear the workspace | Atrium: dismiss active Workfields | "Cleared the board, sir. They're held if you need them back." |

**Where the translation vocabulary lives:** This is **knowledge in Arthur, not chrome in the UI.** The mapping between foreign idioms and native actions is a curated vocabulary table — part of Arthur's prompt context or system knowledge. It doesn't require UI elements; it requires Arthur to recognize "Task Manager" ≡ `InspectProcess`, "Control Panel" ≡ config domain, etc.

**Grounding in shipped/ratified substrate:**
- **OT-26 D1 (constrained decode):** The grammar already constrains Arthur to valid actions or clarify. The translation layer feeds the intent parser — "Task Manager" maps to `InspectProcess` before the grammar constraint applies.
- **OT-26 D4 (clarify-don't-guess):** When the foreign idiom is ambiguous ("open the settings"), Arthur clarifies: "Which settings, sir — display, network, or something else?" This is the translation layer's error correction.
- **OT-44 H2 catalog (manifest-visible app catalog):** The catalog IS our equivalence database. When a user drops a .exe or asks for an app by its Windows name, Arthur queries the catalog for a native equivalent with matching capability declarations. The caps manifest tells Arthur what each app *does*, enabling semantic matching ("the user wants photo editing" → GIMP in the catalog declares image-editing caps).
- **OT-46 butler register:** The gentle naming ("that's `neo> ps` here, sir") is VISION rule 2 (report the outcome) + the explain-never-vanish lens from OT-50.

**What the translation vocabulary needs to cover (initial set):**
- Windows: Task Manager, Control Panel, Recycle Bin, Start Menu, File Explorer, Registry Editor, Device Manager, Disk Cleanup, System Restore
- macOS: Finder, Spotlight, Activity Monitor, System Preferences, Launchpad, Time Machine, Dock, Mission Control
- Generic: "install," "uninstall," "search," "print," "screenshot," "restart," "shut down"

**Scope:** Start with ~30 high-frequency idioms. Curated, not scraped. Each entry maps foreign phrase → intent category → native ActionKind or catalog query. Guarded by clarify-don't-guess: if the mapping is low-confidence, Arthur asks rather than guessing.

### The arrival moment: First conversation = zero learning curve

**Zorin's mechanism:** Familiar desktop layout on first boot.
**The principle:** The first five minutes cost nothing because the user already knows what to do.

**LogansOS-native transposition:**

Our first five minutes = the first conversation. The v1 console greeting (VISION Example 10) already establishes this:

> **Arthur:** Good afternoon. I'm Arthur — I look after this machine for you. Tell me what you'd like, in your own words, and I'll see to it. If ever you'd rather do it yourself, "help" shows you the way.

**Why this beats Zorin's approach for a conversation-first OS:**
- Zorin wins the first five minutes by making the screen look familiar. We win it by making *nothing need to be learned.* Everyone already knows how to talk. A Windows migrant says "find my files" and Arthur does it. A macOS migrant says the same thing and Arthur does it. The vocabulary is their own.
- **The familiarity translation layer makes this stronger:** Even if the migrant uses OS-specific terms ("open Finder," "show me the Start menu"), Arthur understands the intent and acts. There's no desktop metaphor to learn because there is no desktop — just conversation.
- **F1-F8 applies directly:** F1 (can a five-year-old form the request?) — yes, they can say what they want. F8 (is `neo>` available?) — yes, "help" names the deterministic path from the first greeting.

**What must happen in the first conversation to earn "zero learning curve":**
1. The greeting must invite natural language (VISION Ex. 10 — done).
2. Arthur must understand common foreign idioms on first utterance (translation layer — new direction).
3. If the user says something Arthur can't map, it must clarify, not error (D4 — shipped).
4. The first successful interaction must produce a visible result in the transcript (P2 from F1-F8 — narrated state).

### The honest non-transposition: What we do NOT adopt

**Layout mimicry of other OSes — rejected.** The Atrium is explicitly not a windows-icons-menus desktop (OT-52 ratified paradigm). Borrowing another OS's visual identity is the anti-pattern our design language names. The Tahoe distinctness lesson applies: distinctness IS recognition. Our interface is conversation + the Atrium's Workfield paradigm. Mimicry sets compat expectations a non-Windows OS structurally can't meet — the ReactOS-treadmill rhyme (OT-44).

**Why we don't need it:** Zorin needs layout mimicry because its primary interface IS the desktop metaphor. If the user doesn't recognize the desktop, they're lost. Our primary interface is conversation. The user recognizes their own language. We don't need to look like anything else because we communicate in the one vocabulary everyone already has.

---

## ISC-4 — The anti-lessons (trap-mine the model)

### Anti-lesson A1: The uncanny-valley compat expectation

**Zorin's cost:** Looks like Windows → user expects Windows behavior → disappointment where it can't deliver (file system semantics, app compatibility, driver model, registry vs config). Zorin's own docs caveat Wine compatibility: *"not all Windows apps may be fully compatible."* The gap between appearance and behavior is the complaint surface.

**Our discriminator:** We never promise we're something we're not. Arthur speaks plainly about what LogansOS can and can't do. The explain-never-vanish lens (OT-50) prevents the uncanny valley: when Arthur can't do the Windows thing, it explains why and offers the native path. No borrowed identity to disappoint.

### Anti-lesson A2: Mimicry as permanent identity vs bridge

**Zorin's cost:** The Pro page sells layouts as "macOS-like," "Windows 11-like." This trains the buyer to expect ongoing parity. Zorin must continuously update layouts as Windows and macOS change their UI — a treadmill with no finish line. The user who switches from Windows-like to GNOME-like has graduated; the one who stays on Windows-like forever is locked into borrowed identity.

**Our discriminator:** The Atrium's ratified identity (OT-52). We are what we are. The familiarity translation layer is a *bridge* — it meets the user in their vocabulary and gently introduces ours ("that's `neo> ps` here, sir"). The foreign idiom fades because the native name is offered each time. No borrowed clothes forever.

### Anti-lesson A3: The equivalence-suggestion DB's maintenance burden

**Zorin's cost:** The alternatives page is hand-curated, ~25 entries, last updated May 2026. Each new popular Windows app requires manual addition. The pre-installed annotations require tracking edition-specific package lists. It's a documentation tax that grows with the Windows app ecosystem.

**Our discriminator:** The OT-44 H2 catalog is **semantic, not lexical.** Apps declare what they *do* (capability manifest), not what they're *called*. Arthur matches intent to capability, not name to name. When a new native app enters the catalog declaring photo-editing caps, it automatically becomes a candidate for "I need Photoshop" — no manual mapping required. The catalog's declarative structure is the maintenance-free equivalence database.

### Anti-lesson A4: What Zorin Pro gates vs our tiers-never-the-brain ruling

**Zorin's cost:** The macOS-like and Windows 11-like layouts are Pro-only ($39). This means: if you're migrating from macOS, you can't get the "zero learning curve" experience without paying. The headline usability feature is partially paywalled. Zorin also gates the professional creative suite behind Pro — 20+ apps that a creative professional would need on day one.

**Our discriminator (PL ruling, OT-50):** **The brain is NEVER the tier axis.** The intelligence is always the best the machine can run. Zorin's Pro tier gates layouts and apps — both are *chrome*, not *competence*. Our analog must never gate Arthur's capability behind a tier. The familiarity translation layer, the redirect pattern, the clarify behavior — these are Arthur's core competence. They ship in every edition. If we ever need tiers (OT-50 P6: only on non-brain axes), they'd be about optional subsystems or visual feature profiles, never about what Arthur can understand or do.

**3-vote verification on the Pro-gating claim:**
- **Vote 1:** macOS-like layout is Pro-only — verified on zorin.com/os/pro compare table. A macOS migrant gets a worse first-boot experience on Core. ✓
- **Vote 2:** The creative suite (GIMP, Blender, Kdenlive, etc.) is Pro-only — verified on the same table. A creative professional needs to pay before they can work. ✓
- **Vote 3:** The 4 Core layouts (Windows-like, Touch, Windows List, GNOME Shell) are free — a Windows migrant gets the full "zero learning curve" on Core. The paywall is asymmetric: macOS migrants pay, Windows migrants don't. ✓

---

## ISC-5 — The north-star artifact: the review we want to earn

### The paragraph

> I've been calling LogansOS the most intuitive operating system I've ever used for some time. It's the ideal OS for anyone who's tired of learning computer metaphors — you just tell it what you want, in your own words, and it does it. There's no desktop to learn, no menus to navigate, no commands to memorize. One reason LogansOS stands above the rest is that it understands you even when you speak in Windows or macOS terms — you say "open Task Manager" and it shows you what's running, gently naming what things are called here. Zero learning curve because everyone already knows how to talk. Another reason is that it never hits a dead end — if you ask for something it doesn't have, it suggests what it does and offers to make it undoable. It feels like the computer finally learned your language instead of forcing you to learn its.

### The mechanism checklist that earns each sentence

| Sentence | Mechanism | Status |
|---|---|---|
| "the most intuitive operating system" | F1-F8 gate passed on all Arthur surfaces; P1-P7 from five-year-old THINK embodied | **Partial** — F1-F8 is the gate, not the proof. Needs real-user validation against the checklist. |
| "you just tell it what you want, in your own words, and it does it" | OT-26 A1-A3 shipped (read/nav/multi-step agentic loop). Constrained decode + cap-check. | **EARNED for read/nav** — A1-A3 ship this for file operations. Unearned for arbitrary intent (needs broader ActionKind coverage). |
| "no desktop to learn, no menus to navigate, no commands to memorize" | Conversation-first paradigm (Article 1.5). v1 console: transcript + `neo>`. | **EARNED** — v1 IS text-only conversation. The Atrium (OT-52) preserves this when GUI arrives (intent is additive, not replacement — Fork #3 ratified). |
| "understands you even when you speak in Windows or macOS terms" | **Familiarity translation layer** — curated foreign-idiom → intent mapping in Arthur's knowledge | **NOT YET EARNED** — this is the new OT. Requires: (1) curated vocabulary table (~30 high-frequency idioms), (2) wiring into OT-26 intent parser, (3) butler-register naming of native equivalent. Build: host-only Arthur prompt/knowledge change + a few new ActionKind aliases. |
| "you say 'open Task Manager' and it shows you what's running" | Translation layer maps "Task Manager" → `InspectProcess` action | **NOT YET EARNED** — specific instance of the translation layer above. `InspectProcess` action exists in A1; needs the foreign-vocabulary trigger. |
| "gently naming what things are called here" | Butler register: "that's `neo> ps` here, sir" — VISION rule 2 + explain-never-vanish lens | **PARTIAL** — butler register is ratified (OT-46). The specific "naming the native term" pattern needs to be a standing butler obligation for translation-layer hits. |
| "never hits a dead end" | Redirect pattern: clarify-don't-guess (D4) + catalog suggestion (OT-44 H2) + Wine-equivalent fall-through | **PARTIAL** — D4 is shipped. The catalog suggestion path needs the OT-44 H2 catalog to be populated. The "suggest what it does have" behavior needs wiring from Arthur's deny path into the catalog query. |
| "offers to make it undoable" | OT-19 reversibility: "yes — and I made it undoable" | **EARNED** — A2 shipped copy-before-write + tombstone. Butler register mentions reversibility ("kept aside for the session"). |
| "the computer finally learned your language instead of forcing you to learn its" | The entire translation layer + conversation-first paradigm. No foreign vocabulary required. | **EARNED in principle** (conversation-first collapses the gulf of execution per F1-F8 Q1). **NOT EARNED in practice** until the translation layer ships and the ActionKind coverage is broad enough that "tell it what you want" works beyond file read/nav. |

**Sentences we cannot yet earn (and what build earns them):**
1. **"Understands Windows/macOS terms"** → New OT: familiarity translation layer. Host-only, Arthur knowledge. Depends on OT-44 H2 catalog for app-equivalence suggestions.
2. **"Never hits a dead end" (catalog suggestion path)** → OT-44 H2 build: catalog must be queryable by Arthur's intent parser. The deny→suggest wiring is an Arthur userland change.
3. **"Most intuitive" (the superlative)** → Requires real-user validation, not just architecture. The F1-F8 gate is necessary but not sufficient. This sentence earns itself when someone writes it.

---

## ISC-6 — What banks where

### Proposed ledger disposition

| Finding | Destination | Rationale |
|---|---|---|
| **Familiarity translation layer** (foreign idiom → intent mapping) | **New OT row (OT-55 proposed)** | Concrete, buildable Arthur capability. Has an OT-44 H2 catalog dependency. Small host-only slice (curated vocabulary table + intent-parser wiring) with a clear QEMU scenario (drive a foreign-idiom utterance → assert correct ActionKind + butler naming). Fits the OT pattern: direction → THINK → dispatch → build. |
| **The redirect-never-reject principle** (generalized from Zorin's app suggestion) | **Folds into OT-50 four lenses** | It's the explain-never-vanish lens wearing a migration-psychology face. Already standing as a design lens. No new row needed — strengthen the existing lens's description with the Zorin verification. |
| **Anti-lesson bank** (uncanny valley, mimicry-as-identity, equivalence-DB tax, Pro-gating) | **This FINDINGS doc** (standing reference) | These are discriminators, not build directions. They belong here as a trap-mine for future design decisions. Reference from relevant THINKs when the trap is adjacent. |
| **North-star review paragraph + mechanism checklist** | **`docs/NORTH-STAR-2026-07-11-usability-review.md`** (standing doc) | This is the outward-facing sibling of F1-F8. It turns "someone someday writes about us this way" into a falsifiable acceptance bar. Each sentence has a mechanism owner. Revisit at v1-shipped and pre-1.0 to score what we've earned. |
| **Migration psychology principles** (M1-M4) | **Folds into OT-50 four lenses + this FINDINGS doc** | M1 (meet them where they are) → translation layer (OT-55). M2 (first-five-minutes cost) → first-boot greeting (shipped, VISION Ex. 10). M3 (redirect-never-reject) → explain-never-vanish lens (OT-50). M4 (familiarity as bridge) → Atrium identity (OT-52 ratified). |

### OT-55 proposed row (for PL ratification)

> **OT-55 — The familiarity translation layer — Arthur understands Windows/macOS idiom as intent.** The user says "open Task Manager," "where's the Control Panel," "empty the Recycle Bin," drops a `.exe` — Arthur maps the foreign vocabulary to native intent, executes the native action, and gently names the native term in the butler register ("that's `neo> ps` here, sir"). **Mechanism:** curated vocabulary table (~30 high-frequency idioms → intent category → ActionKind or catalog query), wired into OT-26 intent parser before grammar constraint. Guarded by clarify-don't-guess (D4) on low-confidence mappings. App-equivalence suggestions ride the OT-44 H2 catalog's semantic cap-matching (no manual name-to-name mapping — the catalog declares what apps *do*). **Smallest slice:** host-only vocabulary table + 5 test utterances → assert correct ActionKind + butler naming in the transcript. Kill-switch: translation layer off ≡ current Arthur behavior bit-for-bit (foreign idiom triggers clarify instead of direct map). **Depends on:** OT-44 H2 catalog (for app-equivalence suggestions); OT-26 A1 (for ActionKind execution). **Anti-lessons from Zorin:** never mimic the UI (Atrium identity ratified); the translation layer is a bridge that fades (native name offered each time); never gate Arthur's comprehension behind a tier (PL ruling, OT-50).

---

## Primary sources cited

| Source | URL | What it verified |
|---|---|---|
| Jack Wallen ZDNet review | https://www.zdnet.com/article/how-id-rank-distrowatch-top-10-as-linux-expert/ | The exact review text, Wallen's ranking methodology, Zorin at #1 for usability |
| Zorin OS homepage | https://zorin.com/os/ | Product positioning, layout switcher description, pre-installed apps, edition structure |
| Zorin OS Pro page | https://zorin.com/os/pro/ | 12 layouts (8 Pro-only), creative suite, productivity tools, $39 one-time pricing, compare table |
| Zorin technical details | https://zorin.com/os/details/ | Base system (Ubuntu 24.04 LTS), GNOME/XFCE desktops, kernel 6.17, package lists per edition |
| Windows App Support doc | https://help.zorin.com/docs/apps-games/windows-app-support/ | .exe/.msi detection mechanism, Wine installation flow, "check native first" guidance |
| Alternatives to Windows Apps | https://help.zorin.com/docs/apps-games/alternatives-to-windows-apps/ | Curated equivalence database (~25 entries, 5 categories, hand-maintained, last modified May 2026) |
| Install Zorin OS guide | https://help.zorin.com/docs/getting-started/install-zorin-os/ | Installer flow, dual-boot option, first-boot experience (no dedicated tour wizard) |
| Switch Your Organization doc | https://help.zorin.com/docs/getting-started/switch-your-organization-from-windows/ | 5-step migration guide — migration path is a first-class concern |

---

## Gate + close

Docs-only branch — no code changes, no CI run exists (paths-ignored per dispatch). `scripts/worker-close-gate.sh` host set locally; skip-with-note on a docs branch.

**Artifact:** `Plans/RESEARCH-2026-07-11-zorin-usability-FINDINGS.md` (this file).

**RETURN checklist:**
- [x] FINDINGS path with ISC-1 through ISC-6
- [x] Verified mechanism inventory (Zorin Appearance layout-switching: what it changes, what it doesn't, free vs Pro; Windows-app detection + suggestion pipeline: .exe intercept → curated alternatives → Wine fall-through; first-boot arrival experience; Core/Pro edition split)
- [x] Migration psychology separated from mechanism (M1-M4: meet-them-where-they-are, first-five-minutes cost, redirect-never-reject, familiarity-as-bridge vs -identity; where mimicry hits its ceiling)
- [x] Transposition table (Zorin principle → LogansOS-native analog): familiarity translation layer as Arthur knowledge, first-conversation arrival moment, honest non-transposition (no layout mimicry — ratified)
- [x] Anti-lesson bank (A1-A4: uncanny-valley compat expectations, mimicry-as-permanent-identity, suggestion-DB maintenance tax, Pro-tier gating vs tiers-never-the-brain)
- [x] North-star review paragraph + mechanism checklist (7 earned/partial/unearned sentences with mechanism owners and build gaps)
- [x] Proposed ledger disposition (OT-55 for translation layer; lenses fold into OT-50; north-star review as standing doc; anti-lessons in this FINDINGS)
- [x] Cited primary sources (8 sources, all read directly)

**Diff note:** This is a new file on a new branch. No committed artifact to diff against — the dispatch is the spec.
