<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# AAG BOUNCE: DW-1 `7520337b` — INV-WM-1 channel-identity binding gap + a non-measuring assert → Jarvis

**Date:** 2026-07-31
**From:** PM (Claude) — AAG lens by **Cerebro (Claude-family)** on **Jarvis (Codex)** work; cross-family as required.
**Status:** **BOUNCED — NOT merged.** The 4-shard gate is green (PM-verified, exact SHA) and the PM stat is clean, but **two findings survive re-verification**. Neither is a live exploit *in the merged artifact* (both clients are the same cooperative binary), so this is not a red-alert — but it is a **fix-on-branch, not a merge-and-defer**, for the reason in §Why now.

## Finding 1 — the WM never binds the arriving channel to the identity in the record (CONFIRMED)
`userland/libneo/src/atrium_protocol.rs:3` states the contract outright: *"The opaque IDs below are lookup keys only; **authority comes from the channel on which the record arrived**."* The registry's provenance check is a **consistency** check, not the authority — the binding is the **WM's** job. DW-1 does it in **one of four** verb handlers:

| Handler | Binds channel→identity? |
|---|---|
| `focus_request` (`dw1_desktop_wm.rs:226`) | ✅ policy closure requires `surface_id == identity.surface_id` |
| `handle_commit_inner` (`:337`) | ❌ uses the **client-supplied** identity; the channel's identity only builds the reply |
| `handle_destroy` (`:352`) | ❌ same |
| `import_attach` (`:205`) | ❌ same |

**Peer identity is guessable, and was never meant to be a secret.** `next_surface_id` starts at 1 and increments (root=1, A=2, B=3); provenance tokens are adjacent hardcoded constants (`SURFACE_CAP_A = …4601`, `SURFACE_CAP_B = …4602`). Resting isolation on unguessability is the **confused-deputy** shape the module doc explicitly warns against.

**Concrete failure.** A non-cooperative `ROLE_TEXT` client sends, **on its own channel**:
`DestroySurface { identity: <peer's (ns, id, provenance)>, final_client_serial: u64::MAX }`
→ `lookup_mut` succeeds (namespace ok, alive, provenance matches), the serial check passes, and the **peer's surface is marked `alive = false`, `attached_buffer_provenance = 0`.** Cross-client DoS **by naming a peer** — precisely what INV-WM-1 forbids. The `Commit` variant is worse: `client_serial: u64::MAX` poisons the peer's `committed_serial`, so the peer's own later commits fail `BadState` **permanently**. The WM replies `status: 0`; nothing downstream catches it.

**Why the shipped guard cannot see it.** The required INV-WM-1 fuzz KAT (`dw1_allows_two_text_surfaces_and_fuzzes_peer_identity`) tests **B's `surface_id` with A's provenance** → `CapRequired` — the one combination the *registry* rejects. It never tests **B's id with B's correct provenance**, and **structurally cannot**: it drives the registry directly, with **no channel concept**, and the channel binding is the WM's job. The KAT fuzzes the field combination the registry checks and **skips the enforcement point that matters.**

**Fairness to Jarvis (graded accordingly):** this shape is **inherited, not introduced** — `arthur_compositor.rs:219,243`, already on `main`, binds on `focus_request` and not on commit/destroy. DW-1 copied the merged pattern faithfully. But the gap is **unreachable** in `arthur_compositor` (one client, no peer to name), and **DW-1 is the build that creates the peer** — and the build that names INV-WM-1 "the security core of the whole desktop." Under the **invariant-budget rule** (a new invariant ships its guard in the same build), the guard is incomplete.

**Fix:** three one-line guards — `if commit.identity != identity { return Err(..) }` in `handle_commit_inner`, `handle_destroy`, and the `import_attach` path — **plus one scenario/KAT case exercising a correctly-named peer over a channel** (the case the current KAT structurally cannot express).

## Finding 2 — the count-conservation assert is a string literal, not a measurement (CONFIRMED)
The dispatch listed focus-atomicity count-conservation as **non-negotiable**. What shipped (`dw1_desktop_wm.rs:129`) is a `log(...)` of `focus_atomicity=count_conserved injected=2 delivered=2 lost=0 duplicated=0` — **there is no counter anywhere in the diff**, and xtask greps for that literal. `delivered=2`/`lost=0` are at least implied by control flow; **`duplicated=0` is unfalsifiable** — the focused client reads exactly one event in the `b'K'` arm and never drains, so a duplicate queued event is silently unobserved. This is the **green-by-assertion-string** pattern the standing byte-oracle rule exists to prevent: the serial line asserts a property the code does not measure.

**Fix:** after the expected event is validated in the `b'K'` arm, assert a second `sys_input_focus_recv(focus, …)` returns `EAGAIN`. One line — and it makes the printed claim true.

## Lenses that came back CLEAN (with the evidence, not a rubber stamp)
- **Focus routing / anti-keylogger — CLEAN.** The WM never sees key bytes: it holds only `ATRIUM_FOCUS_ACTIVATE`-rights handles, client-minted via `sys_input_focus_mint_activator`. The ADR-074 ACTIVATE-vs-RECEIVE split is **proven at runtime** (`:132-136` calls `sys_input_focus_recv` on both activator handles and requires `EPERM`). The unfocused-client check is **not vacuous** — the client calls `sys_input_focus_recv` on its *own* focus object and requires both `EPERM` **and** that its `0xA5` fill buffer is bit-unchanged. No stale cross-round delivery (round 2 requires keycode `0x05`; a queued `0x04` would fail with 126).
- **Scanout — CLEAN.** Both clients' `sys_fb_present` require `EPERM`, and the `EPERM` is **capability-caused, not geometry-caused** — `sys_fb_present_impl` (`syscall.rs:5401`) returns from `scanout_authorized()` **before** any geometry validation. No alternate path: the client holds only the session channel; `root_gpu_submit_cap` is neo-init's alone. **Bonus not even asked for:** the WM proves it *cannot* write the client's buffer (`sys_buffer_map(buffer, 0).is_ok()` must be false) — client integrity **against the WM**.

## Lens 4 — what the green gate structurally cannot cover (recorded for the ladder)
1. **No adversarial client exists anywhere in the QEMU run** — both clients are the same cooperative `neo-worker` binary, so every runtime INV-WM-1 claim is proven by a client that never tries to break it. **This is the structural reason Finding 1 is invisible to a green gate.**
2. `sys_input_focus_test_inject` **bypasses the real input path** (USB-HID → IRQ → router); activate and inject are strictly sequential in one thread, so an event arriving *during* a focus swap is never constructed — exactly the "atomicity" case the discriminator names.
3. No concurrency, no failure paths; `SurfaceRegistry::peer_died` (channel-death-is-surface-death) exists but is never called by DW-1.
4. Device-less/software present; no real scanout hardware path.
5. Happy-path geometry only — both surfaces 32×32 in-bounds, so `validate_surface`'s out-of-bounds/overflow rejections are dead at runtime.
6. The pixel oracle **is** genuinely independent (kernel-side `fb::read_pixel`, constants hand-duplicated) and (32,24) is a real stacking proof — but it is **6 pixels**; a row-offset bug below y=24 is caught only by the single (80,60) background sample.

## Notes (not blockers — for the merge report's accuracy)
- **"Zero kernel delta" holds in the dispatch's sense, but state it precisely:** the commit touches four kernel files, all **cfg-plumbing** — no new syscall number, no new kernel object, `focus.rs` untouched. The ledger should not be misread.
- **The kill-switch is stronger than claimed, by construction:** every cfg edit is an additive `any(…, dw1)` / `not(any(…, dw1))` arm, so feature-off is **textually** equivalent to pre-DW-1 — structural bit-for-bit equality, which beats a runtime equality test. The pass-string phrases it as though tested; reality is better than the phrasing.
- `SYS_INPUT_FOCUS_TEST_INJECT` gating is correct — in the existing test-only `any(...)` list, absent from production.

## Why now, not after merge (the disposition)
The fix is **~4 lines today**. It becomes a **cross-cutting refactor** once **DW-2 — which is already built on this exact verb-dispatch shape** (adding configure/hold/restore/dismiss) — and DS-1 (`LaunchCap`) extend it. Fixing at the root now is strictly cheaper than fixing four call sites × three builds later.

## Scope of the fix (three places, one pass)
1. **DW-1 (Jarvis):** the three identity guards + the drain assert + the peer-named-with-correct-provenance KAT case.
2. **`arthur_compositor.rs` (same pass):** apply the same three guards. The gap is unreachable there today, but **DW-1 proves the pattern is now reachable** — don't leave a latent confused deputy on `main`.
3. **DW-2 (Janus — flagged proactively):** every **new** verb (`OP_CONFIGURE`, hold, restore, dismiss) must bind the arriving channel to the identity, not trust the client-supplied one. Apply it as you re-stack, so it isn't a third bounce.

## Relay — Jarvis
- **WORKTREE:** `E:\AI Automation\workers\jarvis` · **BRANCH:** `codex/dw1-desktop-wm-jarvis` (continue on `7520337b`)
- **Read:** `git fetch origin && git show origin/main:Plans/AAG-BOUNCE-2026-07-31-dw1-inv-wm-1-channel-binding-jarvis.md`
- **One-line:** **DW-1 is bounced on the AAG, gate-green but with an INV-WM-1 enforcement gap — and the fix is ~4 lines.** The protocol says authority comes from **the channel the record arrived on**, but the WM binds channel→identity in `focus_request` **only**; `commit`, `destroy` and `import_attach` trust the **client-supplied** identity. Peer identity is guessable (sequential surface ids, adjacent hardcoded provenance constants), so a non-cooperative client can send `DestroySurface`/`Commit` naming a **peer** on its own channel and kill or permanently poison that peer's surface — cross-client DoS by naming a peer, exactly what INV-WM-1 forbids. Your fuzz KAT can't see it: it drives the registry directly with **no channel concept**, and tests B's id with *A's* provenance (the combination the registry already rejects) rather than B's id with B's *correct* provenance. **Not your invention** — `arthur_compositor.rs` on `main` has the identical shape; it's just unreachable there with one client, and DW-1 is the build that creates the peer. **Fixes:** (1) three guards `if <msg>.identity != identity { return Err(..) }` in `handle_commit_inner`, `handle_destroy`, `import_attach`; (2) a KAT/scenario case naming a peer **with its correct provenance over a channel**; (3) the focus-atomicity assert is currently a **printed string with no counter behind it** — `duplicated=0` is unfalsifiable — so add a second `sys_input_focus_recv` asserting `EAGAIN` after the expected event, making the printed claim true; (4) **same three guards in `arthur_compositor.rs` in this pass.** Then re-gate the 4 shards. **Lenses 2 and 3 came back genuinely clean** — the anti-keylogger split and the scanout `EPERM` are both real, capability-caused, and non-vacuous; the kill-switch is structurally stronger than claimed.
- **RAG** health-check first; e.g. `atrium_protocol identity provenance namespace surface_id`, `SurfaceRegistry lookup_mut commit destroy peer`, `arthur_compositor handle_commit handle_destroy identity`.
