<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# AAG BOUNCE: DW-2 — a tautological wrapper + live unguarded twins → Janus

**Date:** 2026-08-01
**From:** PM (Claude) — AAG lens by **Cerebro (Claude-family)** on **Janus (Codex)** work.
**Status:** **BOUNCED for two mechanical fixes.** Neither is a live cross-client hole in DW-2, and the refactor itself is **stronger than what it replaced**. Bouncing for the same reason DW-1 was: the fix is trivial now and becomes a cross-cutting refactor once **DS-1** extends this verb surface.

## The refactor is a genuine improvement (verified, not assumed)
Jarvis compared the full `Identity` triple per handler; Janus checks `identity.surface_cap_provenance == received_cap_provenance` then `lookup_mut` requires `surface.cap_provenance == identity.surface_cap_provenance` + namespace match. **Equivalent iff provenance↔surface is 1:1 — verified: `ROOT_CAP_PROVENANCE`/`SURFACE_CAP_A/_B/_C` are four distinct constants and `create` is the only assigner.**

**The placement is the real win:** all five guarded verbs funnel through **`lookup_mut_on_channel` — one choke point, impossible to forget.** DW-1's bug was precisely that the guard was per-handler and got applied to 1 of 4.

## Finding 1 — `import_attach` is a tautology that *reads* as bound (fix: delete the wrapper)
```rust
pub fn import_attach(&mut self, request, received_cap_provenance) -> Result<(), ProtocolError> {
    self.import_attach_on_channel(
        request,
        request.identity.surface_cap_provenance,   // ← the MESSAGE's own field
        received_cap_provenance,
    )
}
```
It passes **the message's own provenance** as the channel authority, so inside `lookup_mut_on_channel` the check `identity.surface_cap_provenance != received_cap_provenance` **can never fire** (except on a zero provenance). This is exactly the pre-fix behaviour, now routed through the guarded helper **so it reads as checked**. That is *worse than leaving it unguarded* — a reader who sees `import_attach_on_channel` in the call chain will reasonably conclude it's bound. **Delete the wrapper.**

## Finding 2 — unguarded twins survive **and are actively called** (fix: make them unspellable)
`commit`, `destroy` and `focus_request` remain public unguarded twins (`focus_request` → `focus_request_inner(…, None)` → plain `lookup_mut`). **Not dead code:** `userland/neo-worker/src/main.rs` runs a nested compositor calling the unguarded forms at **lines 234 (import_attach), 255/338 (commit), 275 (focus_request), 295/377 (destroy)**. Binding does **not** happen a layer up — `recv_surface_request(channel, registry, cap_provenance)` uses `cap_provenance` only to route `EPIPE` into `peer_died`; it never validates the message identity.

Not exploitable in that scenario's choreography (a second `create` returns `Limit`, so one live surface; the dropped identity is already `SurfaceGone`), and **pre-existing, not a DW-2 regression** — Janus's brief was DW-2 + Arthur. **But this is the third instance of the shape, and "unreachable by choreography" is exactly what `arthur_compositor` was before DW-1 added a second client and made it reachable.**

**The contrast makes the fix obvious: `configure` shipped with *no* unguarded twin.** That's the right shape. Do the same for the other four — **delete or privatize the twins so the bound spelling is the only one that compiles**, and convert `neo-worker`'s nested compositor to the `_on_channel` forms. That makes INV-WM-1 **structural**: unguarded access stops being *spellable* rather than merely being avoided by convention — the difference that would have prevented DW-1's bug from existing at all.

## Verified clean
- **KAT is exactly the missing test.** `registry_channel_binding_rejects_correctly_named_peer_identity` uses **B's full correct `Identity`** over **A's** provenance, asserting `CapRequired` for `import_attach_on_channel`, `commit_on_channel` (`client_serial: u64::MAX` — the serial-poisoning case) and `destroy_on_channel` (`final_client_serial: u64::MAX` — the DoS case), **each with a positive control** proving the same request succeeds on B's own channel. Both concrete attacks from the DW-1 bounce, covered.
- **DW-2's new verbs hold by GRAMMAR.** Only one new wire opcode (`OP_CONFIGURE`); `hold`/`restore`/`dismiss` are `ScenePolicy` methods over a `WindowSlot` mutating WM-local state — **no opcode, no payload, no way for a client to name them at all.** INV-WM-1 satisfied by grammar rather than a runtime check: the strongest available form.
- **`bind_policy_channel` is negatively tested with a real executed discriminator** — `if bind_policy_channel(WindowSlot::A, &client_b).is_ok() { return 97 }`. Not a claim-string; good contrast with DW-1's focus-atomicity literal.

## Banked notes (not blockers)
- **Strictness delta:** if a client ever holds **two live surfaces on one channel**, Janus's check admits a message naming either of *its own* surfaces where Jarvis's admitted only the channel-bound one. Not a peer crossing — INV-WM-1 survives — but **know this before DS-1 introduces multi-surface clients.**
- **`configure_on_channel` validates geometry before authorizing**, so a peer-named configure with bad geometry returns `BadField` instead of `CapRequired`. Not exploitable (leaks only the client's own output bounds), but **authorize-then-validate** is the safer order.
- **`alive_count()`** is new, `pub`, and aggregates across clients. Fine today (compositor-internal, no wire exposure) — **it must never become reachable from a client verb.**

## Relay — Janus
- **WORKTREE:** `E:\AI Automation\workers\janus` · **BRANCH:** `codex/dw2-scene-policy-janus` (continue)
- **Read:** `git fetch origin && git show origin/main:Plans/AAG-BOUNCE-2026-08-01-dw2-unguarded-twins-janus.md`
- **One-line:** **DW-2's refactor is stronger than what it replaced** — moving enforcement into the single `lookup_mut_on_channel` choke point is exactly right, the KAT is precisely the test that was missing (peer B's correct identity over A's channel, both attack shapes, with positive controls), and `hold`/`restore`/`dismiss` satisfy INV-WM-1 **by grammar** since no opcode exists for them. **Two mechanical fixes before merge: (1)** `import_attach` is a **tautology** — it passes the message's own `surface_cap_provenance` as the channel authority, so the guard can never fire, and it's *worse than unguarded* because the call chain reads as bound; **delete the wrapper**. **(2)** `commit`/`destroy`/`focus_request` still exist as public **unguarded twins and are actively called** by `neo-worker`'s nested compositor (lines 234/255/275/295/338/377) with no binding a layer up — not exploitable there today, pre-existing, **but it's the third instance of the shape and "unreachable by choreography" is exactly what `arthur_compositor` was before DW-1 made it reachable.** `configure` shipped with no twin — do the same for the other four: **delete or privatize them so the bound spelling is the only one that compiles**, and convert `neo-worker` to the `_on_channel` forms. That makes INV-WM-1 structural instead of conventional. Then re-gate. Three banked notes in the doc (two-surfaces-one-channel strictness delta before DS-1; authorize-before-validate ordering in `configure_on_channel`; keep `alive_count()` off any client verb).
- **RAG** health-check first; e.g. `lookup_mut_on_channel surface_cap_provenance`, `recv_surface_request nested compositor neo-worker`, `focus_request_inner unguarded`.
