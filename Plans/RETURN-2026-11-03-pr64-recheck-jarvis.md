<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 305: #64 setup-menu THINK re-check

Reviewed PR #64 head: `71cd664084ffa8d17586ca215b047451af0b3881` (`dexter/setup-menu-think`).
Earlier review: relay 284, `Plans/RETURN-2026-11-02-setup-menu-review-jarvis.md` on
`origin/codex/setup-menu-review-jarvis`. Read the complete revised THINK, Dexter's appended relay-285
change list, and `git diff origin/main...pr64`. The PR has no comments (there is no newer `PM:` note);
the PR body's three J.c. decisions are the settled requirements.

- **PASS — Earlier FAIL: Network readiness (§3).** `docs/THINK-2026-11-02-setup-menu.md:280` now distinguishes kernel state from userland availability; `:348-352` and `:375-378` hide the page until S4. S1 explicitly rejects a Network-page log line (`:609-616`), and S4 lands the status opcode and page together (`:668-675`). The previous CDC-ECM lease input no longer implies a working wizard status page. The cited baseline's `SYS_NET_STATS` really looks up a socket (`kernel/src/arch/x86_64/syscall.rs:5932-5944` at the reviewed head).
- **PASS — Earlier FAIL: pre-confirm action-log timestamps (§6 #2).** `docs/THINK-2026-11-02-setup-menu.md:182-207` moves confirmation to leaving Time zone, queues earlier page events, and maps their monotonic instants onto confirmed UTC before appending. Back becomes review-only (`:195-198`), avoiding a second write-once confirm. `:557-565` explicitly requires an append API carrying `time_s`; this is a planned change to the existing `now_s()` path, not a claim it already exists. S3 checks append ordering, corrected timestamps, chain verification, the second-confirm control, and the skipped/unconfirmed path (`:649-662`). Independently recomputed with Python `zoneinfo`: New York `2026-11-01 01:59`, fold 0, is `1793512740` (05:59 UTC); the erroneous wall-as-UTC value is `1793498340`, matching the discriminator. This passes as a design re-check; S3 remains an implementation gate.
- **PASS — Settled J.c. choices match the specified behavior.** Language and Keyboard remain hidden until another language/layout exists (`docs/THINK-2026-11-02-setup-menu.md:117-118`, `:328`, `:695`); the wizard runs every Try boot (`:217-222`); Install from Try offers an explicit Restart into the Install boot entry (`:233-245`).
- **PASS — Required prior-art survey remains present and supports the design-only approach.** `docs/THINK-2026-11-02-setup-menu.md:35-46` lists five candidates, approaches, licenses, dated commit pins, and what is borrowed; `:48-102` compares flows and explains the choices. Rechecked all five pinned commit dates through the upstream GitHub API and their license files/source headers; the stated GPL-family restrictions hold, and the branch adds only two Markdown files. No upstream code is vendored. Survey freshness is explicitly dated 2026-11-02, not represented as today's upstream HEAD.

New: `docs/THINK-2026-11-02-setup-menu.md:763-775` still presents the three settled choices as open recommendations; update that editorial status to the PR body's decisions (the operative flow already matches them).

Validation: documentation review and host timestamp arithmetic only; no code changes or QEMU gate run.
