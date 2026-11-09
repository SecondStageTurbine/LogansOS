<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# Relay 279 — PR #15 items 3/6 re-check — Chad

Reviewed head: `baa841ca56fb7e81b591060c72734b212a85d108`.
RETURN branch: `codex/pr15-recheck2-chad`, from `origin/main` at `d92da6479`.
Earlier: relay 261, `Plans/RETURN-2026-11-01-pr15-recheck-chad.md`
on `origin/codex/pr15-recheck-chad`. Locations below refer to the reviewed head.
Seat notes: latest [PM: Seat F](https://github.com/SecondStageTurbine/LogansOS/pull/15#issuecomment-6579739443).

- **PASS — 3, input A: Arthur's failure is contained.** The untaken reply
  expires at `userland/neo-init/src/desktop_session.rs:3085`; its caller now
  breaks to containment at `:1926`, rather than returning out of the desktop.
  `contain_arthur` disarms his reply, marks him gone and kills/reaps him
  (`:3219`), switching to another live client if necessary (`:3238`). The
  loop at `:2415` continues other clients' turns, lock and pointer service;
  `:2496` returns success to Home after they finish. Channel failure uses the
  same exit (`:1944`). Fallback selection excludes dead Arthur (`:3625`).
  The discriminator at `tools/xtask/src/main.rs:17628` requires Arthur's
  timeout and kill, then probe input, six released handles, probe exit and
  the successful Home return in order. This fixes the earlier live-probe plus
  Arthur-commit-without-receive counterexample.

- **PASS — 3, input B: permission lending cannot park indefinitely on Arthur.**
  With `atrium-apps`, the permission branch calls `settle_arthur` then
  `lend_arthur` (`userland/neo-init/src/desktop_session.rs:2228`, `:2234`);
  the blocking `permission::lend` is feature-off only (`:2246`). The adapter
  uses try-send, try-recv and try-grant (`:3147`, `:3151`, `:3158`).
  `userland/libneo/src/atrium_lend.rs:62` implements O/R/grant/A in order,
  checking the deadline on NotReady and serving the callback before retries
  (`:77`). The callback serves secondary turns (`desktop_session.rs:3173`),
  and timeout/wrong-byte failures reach Arthur-only containment (`:2243`).
  `tools/atrium-logic-tests/src/lend_kats.rs:93` tests the original peer that
  takes the permission reply then never takes O; `:109` covers later stalls.
  These are host protocol tests, not an integrated ring-3 permission-stall run.

- **PASS — relay 261's new session-peer leak.** `connect` now closes both
  session ends on spawn failure (`userland/neo-init/src/desktop_session.rs:1431`)
  and handshake failure (`:1454`). A successful client retains `session_peer`;
  `end` and `end_all` include it in the dying handles (`:3742`, `:3854`),
  which `release` actually closes and counts (`:3800`). Arthur's early and
  normal teardown close it too (`:1695`, `:2484`). This fixes the earlier
  repeated launch/exit and bad-buffer-handshake examples.

- **PASS — 6, earlier CP failures re-checked.** The PR body's CP-57/54/52
  table and discriminators remain present. The former unbounded permission
  offer and session-wide Arthur failure are fixed as above; ordinary polling
  still serves other clients and pointer before Arthur (`desktop_session.rs:1928`).
  The body now explicitly discloses the unbounded initial Arthur handshake
  before any secondary client exists, the bounded launch proof, and modal
  Home/permission user waits. This PASS covers the earlier counterexamples
  against those stated qualifications, not an unconditional claim that all
  WM paths always service input. The design's blanket every-send-is-a-try
  wording (`docs/THINK-2026-10-29-ot88-app-model.md:318`) still needs those
  qualifications.

- **New — FAIL (bounded input responsiveness):** `settle_arthur` (`userland/neo-init/src/desktop_session.rs:3111`) and the lend callback (`:3173`) call only `turns` and yield, with no `pointer_tick` or `lock::service`; concrete input: a live probe plus an approved permission request, Arthur takes its reply but refuses O, then move/click the pointer or press Super+L during the five-second lend timeout; secondary requests progress but WM pointer/lock service pauses until containment, contrary to the pointer-service description at `userland/libneo/src/atrium_lend.rs:7`; run the normal input/lock service in these retry callbacks and add an input discriminator (the host KAT counts callbacks only).

- **PASS — validation:** `cargo test -p libneo -p atrium-logic-tests`
  passed (153 tests, including seven lend KATs); neo-init
  `cargo check -Zjson-target-spec --bin neo-init --features
  atrium-apps,desktop-home-boot-test,arthur-actions` passed from its package
  directory; `git diff --check origin/main...pr15` passed. GitHub's API
  independently reports [gate 41447593709](https://github.com/SecondStageTurbine/LogansOS/actions/runs/41447593709)
  completed successfully at the exact reviewed head. QEMU is unavailable
  locally; runtime gate evidence is remote, and the new input is source-traced.
  No PR code changed.
