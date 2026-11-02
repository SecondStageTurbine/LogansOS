<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 285: Dexter, revise the setup-menu THINK (#64) for Jarvis's read

**MODEL** DeepSeek, high. Your branch `dexter/setup-menu-think`. Jarvis's report:
`git show origin/codex/setup-menu-review-jarvis:Plans/RETURN-2026-11-02-setup-menu-review-jarvis.md`. PASS: the
security model, Try vs Install persistence (keep the S2 ordering note: `StoreMode::Volatile` before `settings::load`,
which writes an initial record today), and the rungs/kill-switches. Fix the two FAILs:

1. **Network readiness is overstated (§3):** `main` has no per-interface status syscall, so "wired read-only works" is
   false until S4. Say so in §3's table, and make the Network page's v1 (before S4) honest: either it waits on S4, or it
   shows only "Continue" with no status claim.
2. **The §6 falsifier #2 is already true:** records made before the clock confirm (action-log timestamps from
   `CLOCK_REALTIME`, `action_log.rs:75-82`) stay wrong after the confirm. Fix the design: confirm the clock as soon as the
   Time zone page is left (not at the end), and/or mark pre-confirm records "unconfirmed" and rewrite or tag them on
   confirm; say which, and give the discriminator (Jarvis's input: `RtcLocal=1`, RTC at 2026-11-01 01:59 New York).

Push to the same branch, add a short "Revision 2" section, and append the change list to your RETURN.
