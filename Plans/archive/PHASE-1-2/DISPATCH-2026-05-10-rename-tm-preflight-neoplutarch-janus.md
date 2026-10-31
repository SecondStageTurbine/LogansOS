# DISPATCH: Trademark and namespace pre-flight for project rename to NeoPlutarch

**Date:** 2026-05-10
**Routed to:** Janus (deepseek-v4 deep research)
**Type:** Research dispatch — informational artifact, not legal advice
**Triggering decision:** Project rename from "AltimitNeo" to "NeoPlutarch" to resolve trademark exposure with Bandai Namco's `.hack` franchise (in-universe "ALTIMIT OS" / "ALTIMIT" name).
**Calibration profile:** Somewhat-elevated review intensity per `Plans/PM-NOTE-2026-05-10-janus-first-pass-citation-spot-check.md` §3 — Janus's primary-source citations have spot-checked at high fidelity; GitHub-specific items have been partial. This dispatch's primary sources are government TM databases and public package registries, both of which are exactly the categories Janus has handled accurately to date.
**Turnaround target:** 24–48 hours.

---

## 1. Background and rationale

The project is currently named "AltimitNeo." After v1.2.0 of the bible landed (2026-05-10) committing the project to the Article 28 product-accessibility goal, the project lead identified that "AltimitNeo" carries trademark risk because:

- "ALTIMIT" / "ALTIMIT OS" is the in-universe operating-system name across the Bandai Namco–owned `.hack` franchise (.hack//SIGN, IMOQ tetralogy, .hack//G.U., etc., 2002 onward).
- The project's name is a derivative form ("AltimitNeo" = Altimit + Neo suffix) that would be hard to disclaim if challenged.
- The project's `ALTIMIT-real-world-implementation.md` deep-research artifact (filed 2026-05-05, ~700 lines) explicitly documents `.hack` canon as design inspiration; the paper trail establishes good-faith homage but also makes the inheritance impossible to deny.
- Article 28 commits the project to "real people on common hardware" — which implies eventual public visibility, which is the threshold where TM concerns transition from theoretical to acute.

Project lead picked **NeoPlutarch** as the target rename. This dispatch verifies it is clean before the Prometheus-routed rename PR lands.

The "NeoPlutarch" choice rationale: "Plutarch" is the Latinized name of the Greek historian Πλούταρχος (Ploutarkhos, ~45–120 CE), author of *Parallel Lives*. Etymologically derived from "Pluto-realm" / "chief of riches" — preserves Pluto's-Kiss-Safe canon-thematic association via real Greek-language ancestry rather than via the `.hack` franchise. The "Neo" prefix preserves Article 25.2 subsystem-prefix consistency (`libneo`, `.neom`, `NeoFS`, `neo-init` all stay valid).

This dispatch does NOT seek legal advice. Janus is a research specialist. For maximum legal certainty the project lead would engage an IP attorney; for an open-source project under MIT OR Apache-2.0 with no immediate commercial sale, public-database research is sufficient to identify obvious collisions. The decision boundary this dispatch supports is "is there a collision serious enough to force picking a different name now, before we rename the codebase?"

---

## 2. Primary research scope

### 2.1 Trademark databases — required

- **USPTO TESS** (United States Patent and Trademark Office, Trademark Electronic Search System). Search for:
  - `PLUTARCH` (exact and word-mark)
  - `NEOPLUTARCH`
  - `NEO PLUTARCH`
  - `NEO-PLUTARCH`
  - Filter to relevant Nice classification: **Class 009** (computer software, operating systems, downloadable software) and **Class 042** (software-as-a-service, software design and development services).
  - Note any LIVE registrations or pending applications. DEAD/abandoned registrations are informational but not blocking.
  - Note any registrations in adjacent classes (educational publishing, books, media) that might bleed into software.

- **EU IPO TMview** (European Union Intellectual Property Office). Same search terms, same Nice classes.

- **JPO J-PlatPat** (Japan Patent Office's trademark database). Critical because Bandai Namco is a Japanese company and the `.hack` IP is registered in Japan. Search for:
  - `PLUTARCH` + Japanese transliteration (プルタルコス / プルタルク).
  - All Bandai Namco trademark registrations matching `ALTIMIT`, `アルティミット`, `.hack`, `ドット ハック`, `AURA`, `アウラ`, `MORGANNA`, `モルガナ`, `PLUTO`, `プルート`.
  - Goal: confirm Bandai Namco does NOT hold a `PLUTARCH`-class registration that could create indirect collision.

- **WIPO Global Brand Database** as a cross-jurisdictional sanity check.

### 2.2 Adjacent-name TM exposure — important

Even if `PLUTARCH` and `NEOPLUTARCH` are clean as marks, check:

- **Bandai Namco's `.hack` franchise TM portfolio.** Enumerate the marks they hold for the franchise. Specifically check for any Pluto-themed marks: `PLUTO'S KISS`, `PLUTO AGAIN`, `PLUTON`, etc. The in-universe "Pluto's Kiss" is canonical but may or may not be a registered mark.
- **Disney's `PLUTO` mark** — Disney's Pluto (the dog) is heavily registered. NeoPlutarch is etymologically distinct (Greek historian, not Disney character), but verify there's no Disney registration broad enough to capture "Plutarch" derivatives in software classes. Expected outcome: clean separation, but worth confirming.
- **Microsoft Pluton** — already known to exist as Microsoft's hardware-security processor (AMD/Intel/Qualcomm partnership, 2020+). Verify the registration scope; expected: covers "Pluton" and tightly-bound forms, not "Plutarch" which is a separate root word. Confirm.

### 2.3 Public namespace and package registries — required

- **GitHub organization names** — check availability:
  - `github.com/neoplutarch`
  - `github.com/neoplutarch-os`
  - `github.com/plutarch-os`
  - `github.com/plutarchos` (one-word fusion)
  - `github.com/neoplut`
  - Note any existing orgs/users with these handles, even if inactive.

- **GitHub repository name collisions** — search for repositories named:
  - `plutarch` (any owner)
  - `neoplutarch` (any owner)
  - `plutarch-os`
  - Identify any high-profile (> 100 stars) repositories that would create user-confusion if NeoPlutarch lands at these names.

- **Package registry namespace** — check availability:
  - **crates.io:** `plutarch`, `neoplutarch`, `plutarchos`, `neoplut`
  - **npm:** same set
  - **PyPI:** same set (specifically chase down whether a Python "plutarch" package exists; PM half-recalls one but is not confident)
  - **Go modules:** check via pkg.go.dev for any module paths containing these names

- **Domain availability:**
  - `neoplutarch.org`, `neoplutarch.dev`, `neoplutarch.io`, `neoplutarch.com`
  - `plutarch-os.org`, `plutarch-os.dev`
  - Note registration status (available, registered-and-parked, registered-and-active).

### 2.4 Existing software collision — important

- **Search for software projects named "Plutarch" or "NeoPlutarch."** Beyond the GitHub repo search, check:
  - `osnews.com`, `phoronix.com`, OS-news aggregators for any prior project announcement under these names.
  - Hacker News search for `plutarch` and `neoplutarch`.
  - Wikipedia disambiguation pages for `Plutarch` (separate from the historian) — note any computing/software entries.
  - Existing programming-language tooling: PM half-recalls a Python testing or linting library named "Plutarch" or similar. Confirm or refute.

### 2.5 Stretch goal — fallback name TM-readiness

If NeoPlutarch returns clean, no further action needed. If NeoPlutarch returns collisional, the project lead will need a fallback. To save a second dispatch round-trip, briefly verify the Tier-1 alternates from PM's prior batch:

- **Ferraegis** (Latin *Ferrum* + Greek *Aegis*; coined word). USPTO + EU IPO + GitHub org availability + crates.io/npm/PyPI namespace check. Expected: very clean (coined word with no prior usage).
- **Adamas** (Greek "unbreakable, untamable"; root of *adamant*). Same scope. Expected: some adjacent uses but probably clean in software class.

Stretch goal is ~30 minutes of additional research. If turnaround pressure is real, skip and dispatch a second round if needed.

---

## 3. Output format

Return a research artifact at `Plans/RESEARCH-2026-05-10-rename-tm-preflight-neoplutarch-janus.md` with the following structure:

### 3.1 TL;DR
2–4 sentence verdict: Clean / Partial-collision / Hard-collision.

### 3.2 Findings by jurisdiction and namespace

For each search target in §2 above, report:
- Search query used
- Result count and relevance
- Specific hits with citations (URL, registration number, owner, status, Nice class)
- Verdict for that specific search: clean / partial-concern / hard-collision

### 3.3 Bandai Namco TM portfolio (specific)

Enumerate registered marks Bandai Namco holds matching `.hack`-related terms across US, EU, JP. Confirm absence of `PLUTARCH`-class registrations in their portfolio.

### 3.4 Existing-software collision report

Specific list of any software projects (active or historical) named "Plutarch" or "NeoPlutarch," with category (programming language tool, library, app, OS, etc.) and visibility (star count, downloads, etc.).

### 3.5 GitHub org / package namespace availability

Concrete table: handle/name → status (available / taken / parked) → if taken, owner and last-activity date.

### 3.6 Domain availability

Concrete table: domain → status → if taken, registrant if WHOIS-public.

### 3.7 Recommendation

One of:
- **Clean to proceed.** No critical-jurisdiction TM hit; no high-visibility software collision; primary GitHub/package namespaces available. Project lead can proceed to Prometheus rename dispatch.
- **Proceed with caveats.** Minor namespace adjustments needed (e.g., GitHub org `neoplutarch` taken but `neoplutarch-os` available; primary `plutarch` crate name taken but `neoplutarch` available). Document the chosen handle/namespace strategy.
- **Hard collision — pick another name.** Critical TM hit in US/EU/JP software class, OR active high-visibility software project of the same name in OS/kernel/systems space. Stretch-goal fallback names (Ferraegis, Adamas) flagged for the lead's next pick.

### 3.8 Caveats and limitations

Standard Janus disclaimer language:
- This artifact is research, not legal advice.
- TM databases lag actual filings by weeks; very recent filings may not appear.
- Common-law (unregistered) TM rights exist in the US and may not appear in TESS but can still be enforced by senior users.
- Names that are clean today can become contested if a senior user emerges later.
- For maximum legal certainty, project lead should engage an IP attorney before any commercial use or significant public release.

### 3.9 References

Citations for every claim. Primary sources only (TM database URLs with registration numbers; GitHub URLs with owner/repo; package registry URLs). No secondary sources without primary backing.

---

## 4. Calibration discipline

Per the spot-check verdict in `Plans/PM-NOTE-2026-05-10-janus-first-pass-citation-spot-check.md`:

- **Primary government databases (USPTO, EU IPO, JPO, WIPO):** Janus's primary-source citation discipline has spot-checked at high fidelity. Treat findings as reliable when accompanied by registration numbers and dated citations.
- **GitHub repos and package registries:** Janus's GitHub-specific citation retrieval has been partial in spot-checks (substrate verified, specific content not always retrievable). For GitHub/package findings, prefer concrete URLs and counts over qualitative descriptions; PM may sample-verify any high-stakes claim.
- **Existing-software collision claims:** Treat as elevated-trust if Janus cites a primary source (project README, package registry page, official announcement); treat as somewhat-elevated trust if cited via secondary source (HN thread, blog post). Flag any uncited claims for follow-up.

The calibration step-down to "normal trust" mentioned in the spot-check note's §3 will continue if this dispatch's output passes review without anomalies. This is data point three in the calibration accrual.

---

## 5. Decision rules — how PM triages the output

When the research artifact returns:

- **Result: Clean to proceed.** PM drafts the Prometheus rename dispatch immediately. Project rename to NeoPlutarch is the next governance commit after bible v1.2.0 + ADR-017.
- **Result: Proceed with caveats.** PM consumes the caveat list, picks the recommended namespace strategy (e.g., `neoplutarch-os` GitHub org instead of `neoplutarch`), confirms the strategy with project lead, then drafts Prometheus rename dispatch with the strategy baked in.
- **Result: Hard collision.** PM presents the collision to project lead, requests a new pick from the existing curated batch (Ferraegis, Adamas, Plutarch-without-Neo, or any other from the Tier 1/2 list). If stretch-goal fallback verification was completed, PM can cite that data; otherwise, project lead picks and PM dispatches a second pre-flight.

In all cases the bible v1.2.0 + ADR-017 commits remain pending until rename is decided, since renaming the project also affects how those commits should be authored (see §6 below).

---

## 6. Out of scope for this dispatch

- **Drafting the actual rename PR.** Prometheus territory after this dispatch returns clean.
- **Picking the new name if NeoPlutarch is collisional.** Project lead's call; this dispatch does not make recommendations beyond reporting cleanliness.
- **Bible/ADR commit sequencing.** PM's call after rename verifies; current lean is Prometheus authors three-commit sequence (bible v1.2.0, ADR-017, rename) under the NeoPlutarch name from commit 1, since v1.2.0 hasn't been git-committed yet.
- **Legal review of the homage paper trail in `ALTIMIT-real-world-implementation.md` and the bible preamble's "descended in spirit from Altimit OS" line.** PM lean: keep the homage acknowledgment in docs as referential speech (TM-defensible); do not rename or remove the research artifact. This is an editorial call that doesn't depend on this dispatch.

---

## 7. Cross-references

- `docs/bible.md` v1.2.0 — current state, uses "AltimitNeo" throughout; rename PR will update.
- `docs/ADR-017-wifi-support-strategy.md` — current state, uses "AltimitNeo" throughout; rename PR will update.
- `Plans/AMENDMENT-PROPOSAL-2026-05-10-bible-article-9-and-accessibility.md` — historical record under AltimitNeo name; preserve as-is.
- `Plans/ADR-PROPOSAL-2026-05-10-wifi-support-strategy.md` — historical record under AltimitNeo name; preserve as-is.
- `ALTIMIT-real-world-implementation.md` — research artifact analyzing `.hack` canon; preserve as-is; rename PR does NOT touch this file.
- `Plans/PM-NOTE-2026-05-10-janus-first-pass-citation-spot-check.md` — calibration framework for this dispatch.
- `Plans/RESEARCH-2026-05-09-sc-3b-silent-timeout-prior-art-janus.md` and `Plans/RESEARCH-2026-05-10-sc-3b-release-on-switch-prior-art-janus.md` — Janus dispatch precedents; format reference for the output artifact.

---

**End of dispatch.**
