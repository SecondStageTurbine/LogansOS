# RESEARCH: Trademark and namespace pre-flight for project rename to NeoPlutarch

**Date:** 2026-05-10  
**Research type:** Informational pre-flight; not legal advice  
**Requested by:** `Plans/DISPATCH-2026-05-10-rename-tm-preflight-neoplutarch-janus.md`  
**Search timestamp:** 2026-05-10, America/New_York

---

## 1. TL;DR

**Verdict: Proceed with caveats.** I found no direct public namespace collision for `NeoPlutarch`: the GitHub handle is unclaimed, exact GitHub repository search returned zero hits, npm/PyPI/crates.io returned no package, pkg.go.dev returned zero modules with matching packages, and the checked `neoplutarch.*` domains returned NXDOMAIN plus RDAP 404.

The caveat is that bare `Plutarch` is not clean as a software namespace: it is already used by an active Haskell/Cardano eDSL project, an npm package, a PyPI package, and a 2026 arXiv quantum-computing system paper. Government trademark checks did not reveal a hard collision for `NeoPlutarch`, but several official portals either require credentials/API keys or disallow automated querying; this should be treated as a public-database pre-flight, not a legal clearance opinion.

## 2. Findings By Jurisdiction And Namespace

### 2.1 USPTO / United States

**Queries requested:** `PLUTARCH`, `NEOPLUTARCH`, `NEO PLUTARCH`, `NEO-PLUTARCH`, with attention to Nice classes 009 and 042.

**Method used:** USPTO Trademark Search/TSDR public pages and exact web-index searches. The TSDR data API could not be used without an API key; USPTO documentation says TSDR programmatic access requires registration/API-key use for bulk data. USPTO's current Trademark Search app also blocked scripted direct search endpoint calls during this run.

| Query / record | Result count and relevance | Specific hits | Verdict |
|---|---:|---|---|
| `NEOPLUTARCH`, `NEO PLUTARCH`, `NEO-PLUTARCH` | 0 observable exact hits in public web-index and package/source searches; official USPTO full result count not obtained due access controls | No direct TSDR serial identified | Clean on observed data; official-search caveat |
| `PLUTARCH` | 1 known USPTO hit found | `PLUTARCH`, serial `77732305`, owner Bristol-Myers Squibb Company, status `DEAD/APPLICATION/Refused/Dismissed or Invalidated`, class 042, goods/services: clinical trials and clinical studies relating to pharmaceutical preparations; abandoned 2011-07-18 | Informational only; not a software mark and dead |
| Disney `PLUTO` | Multiple live Disney `PLUTO` marks, not `PLUTARCH` | Serial `77130184`, reg. `3835478`, Disney Enterprises, Inc., live/renewed, class 016; serial `90980804`, reg. `7356054`, Disney Enterprises, Inc., live/registered, class 028; serial `85182816`, reg. `4564452`, Disney Enterprises, Inc., cancelled/invalidated, class 021 | Adjacent famous mark; no direct `Plutarch` derivative or software-class capture observed |
| Microsoft `MICROSOFT PLUTON` | 1 known US application | Serial `88126848`, owner Microsoft Corporation, class 009 application for hardware/security/software goods; observed TSDR status in this run's prior fetch as dead/abandoned. Microsoft Pluton also exists as a real product name and is treated by partners as a Microsoft mark in some contexts | Adjacent term only; `PLUTON` is distinct from `PLUTARCH` |

**USPTO verdict:** No hard USPTO collision found for `NeoPlutarch`. Bare `PLUTARCH` has a dead class-042 pharma research application. The result is not a formal USPTO clearance because direct official full-text result counts for all requested terms were not obtainable through the automated interface without credentials/manual browser work.

### 2.2 EUIPO / EU TMview

**Queries requested:** `PLUTARCH`, `NEOPLUTARCH`, `NEO PLUTARCH`, `NEO-PLUTARCH`, classes 009 and 042.

**Method used:** EUIPO official API Portal and Trademark Search OpenAPI spec. The product page documents a REST `GET /trademarks` endpoint for searches. The EUIPO security page states the APIs require OAuth/OIDC authorization, bearer access token, and `X-IBM-Client-Id`. Direct API tests for exact and wildcard `PLUTARCH`/`NEOPLUTARCH` queries returned `401 Unauthorized` / invalid client credentials.

| Query | Result count and relevance | Specific hits | Verdict |
|---|---:|---|---|
| `NEOPLUTARCH`, `NEO PLUTARCH`, `NEO-PLUTARCH` | Official API count not obtained; public web-index exact searches did not surface a EUIPO record | None identified | No observed collision; official API caveat |
| `PLUTARCH` | Official API count not obtained | None identified from official accessible output during this run | Inconclusive by API; no public-index collision found |

**EUIPO verdict:** No public collision identified, but official TMview/EUIPO search is incomplete without API credentials or manual UI verification.

### 2.3 JPO / J-PlatPat

**Queries requested:** `PLUTARCH`, `NEOPLUTARCH`, Japanese transliterations `プルタルコス` and `プルタルク`, plus Bandai Namco-related terms (`ALTIMIT`, `アルティミット`, `.hack`, `ドット ハック`, `AURA`, `アウラ`, `MORGANNA`, `モルガナ`, `PLUTO`, `プルート`).

**Method used:** JPO's English support page for J-PlatPat trademark search, the J-PlatPat application, and exact public web-index searches. JPO describes J-PlatPat as the official digital library for patents, designs, and trademarks. The JavaScript application exposed internal endpoints, but I did not complete a reliable automated J-PlatPat POST search in this run.

| Query | Result count and relevance | Specific hits | Verdict |
|---|---:|---|---|
| `NEOPLUTARCH` / `PLUTARCH` / Japanese transliterations | Official result count not obtained; exact public web-index searches did not find a Japan trademark record | None identified | No observed collision; J-PlatPat caveat |
| Bandai Namco `.hack` / `ALTIMIT` / Pluto-themed terms | Official owner-portfolio search not completed in J-PlatPat | No Bandai-owned `PLUTARCH` or `NeoPlutarch` hit identified from public searches | No observed collision; Japan-specific manual verification recommended if legal certainty is needed |

**JPO verdict:** No observed collision, but this is the weakest government-database leg because J-PlatPat was not fully machine-queried.

### 2.4 WIPO Global Brand Database

**Queries requested:** `PLUTARCH`, `NEOPLUTARCH`, `NEO PLUTARCH`, `NEO-PLUTARCH`.

**Method used:** WIPO Global Brand Database public documentation and FAQ plus exact public web-index searches. WIPO states the database covers Madrid System marks and participating national/regional collections. WIPO's FAQ states automated querying/downloading is disallowed under its terms of use, so I did not run automated result scraping.

| Query | Result count and relevance | Specific hits | Verdict |
|---|---:|---|---|
| `NEOPLUTARCH`, variants | Official count not obtained; exact public web-index searches did not surface a WIPO record | None identified | No observed collision; manual UI caveat |
| `PLUTARCH` | Official count not obtained | None identified from accessible public-index output | Inconclusive by official UI; no public-index collision found |

**WIPO verdict:** No public-index collision found, but manual UI verification is required for a stronger claim.

## 3. Bandai Namco TM Portfolio (Specific)

The Bandai risk that triggered the rename is real enough on franchise association grounds: `.hack`/`DOT HACK` marks are live in the United States in class 009, and `Altimit` is strongly tied to the `.hack` fiction even if I did not identify a live `ALTIMIT` registration.

| Mark | Jurisdiction/source | Owner | Status | Class / goods | Relevance |
|---|---|---|---|---|---|
| `DOT HACK//SIGN` | USPTO TSDR serial `78195039`, reg. `2978440` | BANDAI NAMCO ENTERTAINMENT INC. | Live/registration issued and active; renewed 2024-10-24 | Class 009; DVDs/animated series, historically video-game/audio goods | Confirms active US `.hack` portfolio in software/media-adjacent class |
| `DOT HACK//G.U.` | USPTO TSDR serial `77101858`, reg. `3496518` | BANDAI NAMCO ENTERTAINMENT INC. | Live/registration issued and active; renewed 2018-09-11 | Class 009; video game discs | Directly overlaps game/software goods class |
| Related `.hack` registrations | USPTO TSDR related-property output on serial `77101858` | BANDAI / Bandai Namco chain | Related registrations listed include `2772705`, `2978440`, `2853634`, and others | Class details not fully fetched for every related record | Confirms broader `.hack` mark family |
| `ALTIMIT` / `ALTIMIT OS` | Public web search plus USPTO/JPO/EU caveat | No official Bandai serial identified in this run | Not established as a registered mark from retrieved official records | N/A | Still a rename risk because of `.hack` canon and project paper trail |
| `PLUTARCH` / `NEOPLUTARCH` held by Bandai Namco | USPTO public records/web-index searches; EU/JPO/WIPO caveats | None identified | None identified | N/A | No observed Bandai collision |
| Pluto-themed `.hack` terms (`PLUTO'S KISS`, `PLUTO AGAIN`, `PLUTON`) | Public web-index searches and USPTO serial checks where known | None identified as Bandai-owned in retrieved official records | None identified | N/A | No observed Bandai-owned Pluto-themed mark blocking `NeoPlutarch` |

**Bandai Namco verdict:** The rename away from `AltimitNeo` is justified. The checked records support `.hack` as an active Bandai Namco class-009 mark family, but I found no Bandai-owned `PLUTARCH`/`NEOPLUTARCH` record. EU and JP portfolio enumeration remains caveated because official automated owner searches were not completed.

## 4. Existing-Software Collision Report

| Name | Project / source | Category | Visibility | Collision assessment |
|---|---|---|---|---|
| `NeoPlutarch` | GitHub repo search, npm, PyPI, crates.io, pkg.go.dev, HN, public web-index exact search | None found | 0 GitHub repo hits; package registries checked clean | Clean on observed public software namespaces |
| `Plutarch` | `Plutonomicon/plutarch-plutus` | Haskell/Cardano typed eDSL for UPLC | 133 GitHub stars; pushed 2026-05-10 | Meaningful collision for bare `Plutarch`; not for full `NeoPlutarch` |
| `Plutarch` | `Plutonomicon/plutarch-core` | Haskell/Cardano core project | 19 GitHub stars; pushed 2025-03-31 | Reinforces bare-name collision |
| `plutarch` | npm package `plutarch` | CLI for serving/building React apps | Latest `3.1.10`; modified 2022-06-24; 264 downloads last month | Bare package name taken |
| `plutarch` | PyPI package `plutarch` | Python client for the Plutarch API | Version `0.1.1` | Bare package name taken |
| `Plutarch` | arXiv:2601.08930, "Plutarch: Toward Scalable Operational Parallelism..." | Quantum-computing execution/scheduling research | Submitted 2026-01-13 | Computing-sector use of bare `Plutarch`; not an OS project |
| `PlutarchOS` | `Ibrahimsheikh02/PlutarchOS` | GitHub OS-named repository | 8 stars; pushed 2026-03-13 | Low-visibility but OS-adjacent; avoid `plutarch-os` as primary brand |
| `Plutarchos` | GitHub user `Plutarchos` | User account | Created 2026-01-15; 1 public repo | Handle collision only; avoid one-word fusion if possible |
| `Plutarch` on Hacker News | HN Algolia query | Mostly historical/literary items | 25 story hits; top results about historical Plutarch/Julius Caesar | No software collision observed on HN |
| `NeoPlutarch` on Hacker News | HN Algolia query | None | 0 story hits | Clean |
| `Plutarch` on Phoronix/OSNews | Exact public web searches | No exact OS/project announcements found | No relevant indexed result | No observed OS-news collision |

**Software verdict:** `NeoPlutarch` is clean; bare `Plutarch` is not. Use `NeoPlutarch` consistently in public-facing materials and prefer `neoplutarch` for package namespaces.

## 5. GitHub Org / Package Namespace Availability

### 5.1 GitHub Handles

| Handle | Status | Owner / activity |
|---|---|---|
| `github.com/neoplutarch` | Available/unclaimed | GitHub API returned 404 |
| `github.com/neoplutarch-os` | Available/unclaimed | GitHub API returned 404 |
| `github.com/plutarch-os` | Available/unclaimed as a handle | GitHub API returned 404, but repository-name collision exists (`Ibrahimsheikh02/PlutarchOS`) |
| `github.com/plutarchos` | Taken | User `Plutarchos`; created 2026-01-15; 1 public repo |
| `github.com/neoplut` | Available/unclaimed | GitHub API returned 404 |

### 5.2 GitHub Repository Name Search

| Query | Result count | Notable hits |
|---|---:|---|
| `plutarch in:name` | 155 | `Plutonomicon/plutarch-plutus` (133 stars, pushed 2026-05-10), `Plutonomicon/plutarch-core` (19 stars, pushed 2025-03-31) |
| `neoplutarch in:name` | 0 | None |
| `plutarch-os in:name` | 1 | `Ibrahimsheikh02/PlutarchOS` (8 stars, pushed 2026-03-13) |
| `plutarchos in:name` | 2 | `Ibrahimsheikh02/PlutarchOS`; `Plutarchos/you_bet` |
| `neoplut in:name` | 1 | `verri/neopluto` (0 stars, pushed 2018-05-19); not exact |

### 5.3 Package Registries

| Name | crates.io | npm | PyPI | Go / pkg.go.dev |
|---|---|---|---|---|
| `plutarch` | Available/not found | Taken: `plutarch` `3.1.10`, React CLI, modified 2022-06-24, 264 downloads last month | Taken: `plutarch` `0.1.1`, "Python client for the Plutarch API" | Search shows 1 module with matching packages, `github.com/blueskan/harpocrates`; not an exact module path |
| `neoplutarch` | Available/not found | Available/not found | Available/not found | 0 modules with matching packages |
| `plutarchos` | Available/not found | Available/not found | Available/not found | 0 modules with matching packages |
| `neoplut` | Available/not found | Available/not found | Available/not found | 0 modules with matching packages |
| `ferraegis` | Available/not found | Available/not found | Available/not found | 0 modules with matching packages |
| `adamas` | Available/not found | Taken: `adamas` `0.0.1`, modified 2023-07-20, 9 downloads last month | Available/not found | 0 modules with matching packages |

**Namespace verdict:** `neoplutarch` is the right namespace. Do not use bare `plutarch` for package publication. `Ferraegis` looks cleaner than `Adamas` as a fallback package namespace, but full trademark clearance was not completed because `NeoPlutarch` did not hard-collide.

## 6. Domain Availability

Domain checks used local DNS resolution plus RDAP at `https://rdap.org/domain/<domain>`. RDAP 404 plus DNS NXDOMAIN is a strong availability signal, but it is not the same as a registrar cart reservation.

| Domain | DNS status | RDAP status | Verdict |
|---|---|---|---|
| `neoplutarch.org` | NXDOMAIN / no A record | HTTP 404 | Likely available |
| `neoplutarch.dev` | NXDOMAIN / no A record | HTTP 404 | Likely available |
| `neoplutarch.io` | NXDOMAIN / no A record | HTTP 404 | Likely available |
| `neoplutarch.com` | NXDOMAIN / no A record | HTTP 404 | Likely available |
| `plutarch-os.org` | NXDOMAIN / no A record | HTTP 404 | Likely available |
| `plutarch-os.dev` | NXDOMAIN / no A record | HTTP 404 | Likely available |

## 7. Recommendation

**Proceed with caveats.**

Use **NeoPlutarch** as the public project name and reserve the `neoplutarch` GitHub organization plus the preferred `neoplutarch.*` domains before announcing. Publish packages under `neoplutarch` where possible. Avoid bare `plutarch` because it is already used in multiple software contexts, and avoid `plutarch-os`/`PlutarchOS` as the primary project identity because there is a low-visibility but direct OS-named GitHub collision.

The rename from `AltimitNeo` remains prudent: Bandai Namco has live `.hack`/`DOT HACK` class-009 marks, while `NeoPlutarch` has no observed direct Bandai or package-namespace collision. For commercial launch, fundraising, merchandise, or broad public release, have an IP attorney or a human trademark searcher run manual USPTO/EUIPO/JPO/WIPO clearance before relying on this report.

## 8. Caveats And Limitations

This artifact is research, not legal advice.

Trademark databases lag actual filings; very recent filings may not appear. Common-law trademark rights in the United States can exist without a federal registration and may not appear in USPTO search results.

Government database coverage is uneven in this run. USPTO TSDR status pages were readable by serial number, but direct full-text automation through USPTO Trademark Search/TSDR API was blocked by API-key/WAF controls. EUIPO's API requires OAuth/client credentials. WIPO's FAQ disallows automatic querying/downloading. J-PlatPat was inspected, but a reliable automated trademark POST search was not completed.

No trademark search can prove permanent safety. Names that are clean today can become contested later if a senior user emerges or if the project moves into commercial channels.

## 9. References

### Government and Trademark Sources

- USPTO Trademark Search information: `https://tmsearch.uspto.gov/search/search-information`
- USPTO TSDR Data API documentation: `https://developer.uspto.gov/api-catalog/tsdr-data-api`
- USPTO TSDR API key FAQ: `https://developer.uspto.gov/faq/tsdr-api-bulk-download`
- USPTO TSDR status, `PLUTARCH`, serial `77732305`: `https://tsdr.uspto.gov/statusview/sn77732305`
- USPTO TSDR status, `DOT HACK//SIGN`, serial `78195039`: `https://tsdr.uspto.gov/statusview/sn78195039`
- USPTO TSDR status, `DOT HACK//G.U.`, serial `77101858`: `https://tsdr.uspto.gov/statusview/sn77101858`
- USPTO TSDR status, Disney `PLUTO`, serial `77130184`: `https://tsdr.uspto.gov/statusview/sn77130184`
- USPTO TSDR status, Disney `PLUTO`, serial `90980804`: `https://tsdr.uspto.gov/statusview/sn90980804`
- USPTO TSDR status, Disney `PLUTO`, serial `85182816`: `https://tsdr.uspto.gov/statusview/sn85182816`
- USPTO TSDR status, `MICROSOFT PLUTON`, serial `88126848`: `https://tsdr.uspto.gov/statusview/sn88126848`
- EUIPO Trademark Search API product page: `https://dev.euipo.europa.eu/product/trademark-search_100`
- EUIPO API security/OAuth page: `https://dev.euipo.europa.eu/security`
- JPO trademark search support page: `https://www.jpo.go.jp/e/support/j_platpat/trademark_search.html`
- J-PlatPat official application: `https://www.j-platpat.inpit.go.jp/`
- WIPO Global Brand Database overview: `https://www.wipo.int/reference/en/branddb/`
- WIPO Global Brand Database FAQ / automation limitation: `https://www.wipo.int/en/web/global-brand-database/faqs_branddb`

### Software and Namespace Sources

- GitHub user API pattern: `https://api.github.com/users/<handle>`
- GitHub repository search API pattern: `https://api.github.com/search/repositories?q=<query>&per_page=5`
- `Plutonomicon/plutarch-plutus`: `https://github.com/Plutonomicon/plutarch-plutus`
- `Plutonomicon/plutarch-core`: `https://github.com/Plutonomicon/plutarch-core`
- `Ibrahimsheikh02/PlutarchOS`: `https://github.com/Ibrahimsheikh02/PlutarchOS`
- GitHub user `Plutarchos`: `https://github.com/Plutarchos`
- npm package API/page, `plutarch`: `https://registry.npmjs.org/plutarch`, `https://www.npmjs.com/package/plutarch`
- npm downloads API, `plutarch`: `https://api.npmjs.org/downloads/point/last-month/plutarch`
- npm package API/page, `adamas`: `https://registry.npmjs.org/adamas`, `https://www.npmjs.com/package/adamas`
- PyPI JSON/page, `plutarch`: `https://pypi.org/pypi/plutarch/json`, `https://pypi.org/project/plutarch/`
- crates.io API pattern: `https://crates.io/api/v1/crates/<name>`
- pkg.go.dev search, `plutarch`: `https://pkg.go.dev/search?q=plutarch`
- pkg.go.dev search, `neoplutarch`: `https://pkg.go.dev/search?q=neoplutarch`
- Hacker News Algolia search API: `https://hn.algolia.com/api/v1/search`
- arXiv `Plutarch` paper: `https://arxiv.org/abs/2601.08930`
- RDAP domain lookup pattern: `https://rdap.org/domain/<domain>`

---

**End of research artifact.**
