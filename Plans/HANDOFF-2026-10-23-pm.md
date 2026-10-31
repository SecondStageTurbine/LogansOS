<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# PM HANDOFF — 2026-10-23 ~02:00 UTC (outgoing PM session 014Q7XRAeCXPYSRV5C6bczYE)

You are the next Claude PM for LogansOS (repo `SecondStageTurbine/LogansOS`, MCP name `secondstageturbine/neoplutarch`).
Read CLAUDE.md first (it is binding and was updated 2026-10-21/22 with the PL's review-routing rules). This file is the
live state. **main = `334f5ba4`** when written.

## How the PM works here (the parts CLAUDE.md doesn't spell out)

- **Sessions.** Coders are Claude cloud sessions. Message them with `create_trigger` using `persistent_session_id` +
  `run_once_at` (a few minutes ahead; it must be in the future). They post "PM: ready to gate <SHA>" /
  "PM: gate report" as PR comments; read those with `pull_request_read get_comments`.

  | Session | ID | Current lane |
  |---|---|---|
  | S1 | `session_01Q33EbDudjhFcs7UvK2nFUg` | Zenbook model-load freeze fix |
  | S2 | `session_01YcKsU89NhUm69dR2jd4aPD` | USB #93, watchdog #95 |
  | S3 | `session_01Jxk9xeUi5AspqT3YMAJzjB` | idle after i8042 #98 merged |
  | S4 | `session_01DguBjae7CX9xGyKvMdwAHB` | idle (wall-clock done) |
  | S5 | `session_01HJJaAB9nnFgtpVvdx56VHc` | ACPI/AML (OT-86) |

- **Codex/Muse seats** (Jarvis, Janus, Chad, Muse/Gwen) are reached only through the PL. You write a relay file and
  send it to J.c.; he pastes back a RETURN SHA.
  - Relay format: `MODEL:`/`EFFORT:` line plus one-line why, then WORKTREE, BRANCH, the read commands, a plain
    pass/fail checklist, and "commit RETURN … PUSH it, paste the SHA".
  - The last relay was **#30**, so the next is **#31**.
  - Routing:
    - core SMP/timer/memory/syscall: GPT-6.1-Sol high;
    - narrow re-check: 6.1-Sol medium;
    - contained: GPT-6-Luna high;
    - light: GPT-5.6-Luna medium;
    - Daybreak Blue if refused;
    - GPT-6-Astra after 2+ bounces;
    - security reviews of Claude code go to Muse.
- **Gates.**
  - Dispatch: `actions_run_trigger run_workflow ci.yml ref=<branch>` (the 4-shard QEMU gate). Max 2 at once; a run
    takes about 1–1.5 h.
  - Before merge:
    - the PR base must be **main** (stacked PRs bit us once: #84 merged into its dead stack base and had to be
      re-landed as #90);
    - the head SHA must equal the gated SHA;
    - if main moved since the gate, check overlap with `git merge-tree --write-tree origin/main origin/<branch>` plus
      the shared-file list. Docs-only or disjoint and clean means the gate transfers; otherwise the coder merges main
      in and you re-gate.
  - Merge with squash + `expectedHeadSha`, then verify the stat and **cancel the main push run** (it's redundant and
    wastes runners).
  - Infra flakes (crates.io HTTP2, DNS, runner loss) get `rerun_failed_jobs` once.
- **Git proxy.** This cloud session can push only its designated branch `claude/phase-4-continuation-TlW3j`, and
  can't delete other branches. Docs to main go through a PR from that branch, or `create_or_update_file` on main.
  **That branch currently holds `acpi-drop/`** (the PL's raw ACPI dumps, MSDM/SLIC stripped). S5 reads from it, so
  don't reset it until S5 confirms all sets are ingested, and **never merge `acpi-drop/` to main**.
- **Heartbeat.** The trigger `trig_01PZZgvr8AMU7aaiqNDAJsi6` belongs to the OLD session. Create your own heartbeat
  in your session.
- **The PL (J.c.).** Plain language, EDT times. Summarise PL-reserved lanes in ONE message: per-gate pass/fail, the
  branch, and the single decision.
  - The Zenbook is his WORK laptop: **NEVER write its internal disk**. Photos of its screen are valid evidence.
  - The Kingston stick is the image stick.

## Lane state

### Merged since 2026-10-21

- #84→**#90** LAPIC timer.
- #85, #88 write guard.
- #86 browser C0.5.
- #87 wall-clock.
- #89, #94 docs.
- #91 lapic-timer in images.
- #92 OT-83: a ring-3 exception kills the process, not the machine.
- #97 OT-83 follow-up.
- #98 i8042 PS/2 keyboard.

### Open

1. **#93 Zenbook USB input (S2), head `516155dc`.**
   - Review: complete. Chad b0f9c314 BOUNCE → f20f9e38 (1 open) → **0552ae47 CLOSED**. The USB code is unchanged
     since the reviewed `3829b9f1` (S2 showed an empty diff).
   - **Gate run 39881531112 was in progress.** MERGE ON GREEN (base main 334f5ba4).
   - Then tell S2. S2 retargets **#95** (the freeze watchdog overlay, stacked on #93) to main, refreshes it, and posts
     "ready". Gate #95; it needs only a PM review (framebuffer/atomics, no locks).
2. **The combined Zenbook test image**, after #93 (and #95) are on main.
   - Relay Janus to build image A (`cargo xtask iso --with-model <pinned .neom>`) from main or #95's head:
     USB + i8042 + watchdog.
   - The PL tests:
     - the built-in keyboard (i8042);
     - the USB keyboard/mouse (the USB-A port, behind a hub);
     - the freeze, by photographing the watchdog overlay.

   Previous Janus images:
   - `0294942e` (main 4e9ea28c);
   - `a4119c32` (USB test 6f09e826): **the USB kb/mouse WORKED on the Zenbook**;
   - `d9ad3424` (watchdog d9d766ab).
3. **The Zenbook model-load freeze, now S1's new lane** (branch `claude/zenbook-model-load-freeze`, nothing posted yet).
   - CONFIRMED by watchdog photos: after asking Arthur, `cpu0 ticks/s=0`, `model load-build weight 0/0`, and Arthur
     inside syscall 0x110 for 29 s+. So `sys_model_load` runs at IF=0 and never reaches the weight build; the WM
     blocks in its G/M handshake. On the OptiPlex an answer took 3–4 min; on the Zenbook it hung for over 15 min.
   - Diagnosis note: `origin/claude/zenbook-ask-freeze:Plans/NOTE-2026-10-22-zenbook-ask-freeze.md`.
   - Fixes:
     - load once at boot, or preemptibly;
     - blake3 at opt-level 3;
     - the WM handshake polls the pointer;
     - find why it never finishes.
   - Review: **Jarvis, GPT-6.1-Sol high**. Normal PM merge.
4. **#96 ACPI/AML rung 0 (S5).** **J.c. said "Acpi is a go"** (2026-10-23 01:12 UTC): `acpi` 6.1.1 is the Loganfy
   base, and he gave the go-ahead to merge #96.
   - Janus review `e20f52fc`: 2 FAILs, 6a (wording overstates corpus completeness) and 7a (Windows `\` paths in the
     report; normalise to `/`).
   - S5 is fixing both, recording the decision, and ingesting the OptiPlex **Linux** set (`dell-optiplex-7080-linux`).
     Its DSDT is byte-identical to the Windows dump, so the OptiPlex DSDT failure is a CRATE bug.
   - On S5's "PM: ready", **you verify the two fixes yourself** (no new cross-family pass): no kernel, userland or
     xtask diff; host CI green (PR CI triggers only on open/reopen; close and reopen the PR to kick it). Then MERGE.
   - Post to S5 that #96 merged so Fix 1 can start.
5. **ACPI Fix 1 (S5, after #96).** J.c.'s full brief is in my trigger to S5. The method:
   1. **Rebaseline first**: run the unmodified crate on the corpus including the new OptiPlex Linux set.
   2. Fix the value-kind confusion at the semantic root, citing the spec for each pattern.
   3. Gates (from the rebaseline; the old figures were 71, 24, 3/6 and 305/397):
      - OptiPlex DSDT loads;
      - 71 → 0;
      - QEMU wrong answers 24 → 0 (J.c.: "the number to watch");
      - no regressions;
      - no new panics.
   4. Stop and report on any spec-vs-ACPICA divergence.

   **NO MERGE without J.c.** Needs a cross-family review (6-Luna high). Next come Fix 2 (the 18 crash cases become
   fail-closed errors, a Loganfy invariant) and Fix 3 (Zenbook I2C/GPIO `_CRS` and the UUID `_DSM` → touchpad/touchscreen).
6. **Driver plan:** `Plans/NOTE-2026-10-22-driver-coverage.md`.
   - Done: i8042.
   - Next after ACPI fixes: I2C-HID (touchpad/touchscreen), RTL8125, HDA.
   - Main PC / Zenbook inventories: kept in the private repo (formerly `docs/hardware/`).

## Owed docs PR (bank RETURNs, record milestones)

- **RETURNs to bank** (they're on codex/* branches; add them under `Plans/`):
  - Chad: b0f9c314, f20f9e38, 0552ae47;
  - Jarvis: 839d2795 (OT-83 recheck), be54d46b (i8042);
  - Janus: a4119c32, d9ad3424, and the next image RETURN.
  - (60de60e6 and 0294942e were banked in #94.)
- **Milestones to record** (OPEN-THREADS / release ledger):
  - Zenbook S14 boots image A to the desktop on silicon (x2APIC + LAPIC TSC-deadline timer + RTC correct);
  - USB kb/mouse work behind a hub on the south xHCI 00:14.0;
  - the model-load freeze is diagnosed by watchdog.
- The ACPI corpus covers 6 PL machines + QEMU. The M710q was not delivered. Partial Windows dumps: HP 3, M910q 2,
  MSI 1 missing (FACS is never exported by Windows). Re-dumping those is optional (J.c. asked; the PM said not needed).

## PL is not waiting on anything right now

When #93 (and #95) merge, the next thing he sees is the combined-image relay, then the S1 freeze-fix review relay
(Jarvis), then the ACPI Fix 1 gate report.
