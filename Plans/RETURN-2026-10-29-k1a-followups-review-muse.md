<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# RETURN — relay #151: #197 `cbe57444`, your #166/#168 follow-ups

Reviewer: Muse (Gwen), a separate model family, reviewing Claude-authored code. Pasted back by the PL on 2026-10-29 and
banked by the PM verbatim. **PL verification:** J.c. checked the load-bearing claims himself at `cbe57444`:
- `Derivations` is a tuple struct with a private field;
- `insert_without_cap_graph` is cfg-gated to the negative-test feature, with a single gated call site;
- that feature is `[]` and absent from defaults;
- `retire` clones guard-pinned Arcs (CP-91);
- the ADR carries the "wording corrected 2026-10-29" parenthetical.

Files read from the repo at the reviewed SHA, not from main and not from a summary. Re-checks the follow-up items from
the K1a-0 RETURN (relay #108, PR #166) and the K1a root-dir RETURN (relay #111, PR #168), both fetched from main and
read. The `is_directory()` observation from #111 item 5 was already fixed by #174 (`57b7960e`) and is not
re-litigated here.

Reviewed SHA: `cbe57444` (PR #197). Files: `kernel/src/sched/process.rs` (`process::edges`, `CapGraph`, `retire_one`,
`teardown`, `install_cap`), `kernel/src/sched/cap_graph_logic.rs` (pure re-parent, unchanged),
`docs/ADR-010-capability-system-foundation.md` (K1a-0 amendment, K1b paragraph), `tools/cap-graph-logic-tests/src/lib.rs`
(edge gate), `kernel/Cargo.toml` (feature gating).

1. No code outside `process::edges` can reach the derivations map or change an edge without a `CapGraph` guard — PASS.
   Every method of `Derivations` (`kernel/src/sched/process.rs:1959-2030`) was checked:
   * The map itself is unreachable: `Derivations` is a tuple struct whose single field
     (`SwitchMutex<BTreeMap<CapId, Option<CapId>>>`) has no `pub`, so no code outside `edges` can name, read, or write
     the inner map — including through the `pub derivations` field on `Process` (`:426`), which exposes the wrapper
     value only. A macro expanding to a write, an alias under another variable name, a helper in another module, and a
     direct field reassignment all fail at the same place: there is no constructible handle to the map. The only
     constructions are `Derivations::new()` in the two `Process` constructors (`:646`, `:667`); no reassignment site
     exists anywhere in the file (grep for `derivations.0` finds nothing outside the methods themselves).
   * Every mutating method is `pub(super)` — callable only from within the `process` module — and takes `&CapGraph` (a
     proof value constructible only via `lock_cap_graph()`, `:2050`, since `CapGraph`'s fields are private and its only
     literal construction is `:2058`): `insert`, `remove`, `remove_all`, `take`, `reparent`, `for_each`. `CapGraph`
     cannot be forged: there is no public constructor and no `Default`.
   * The test-only methods are read-only and cfg-gated: `is_empty_for_test` and `parent_for_test` are `pub` but under
     `#[cfg(feature = "neo-test")]`, and they only read (`is_empty`, `get().copied`); they cannot change an edge. The
     one write without a guard is `insert_without_cap_graph`, which is item 2's deliberate, cfg-gated exception, not a
     general path.
   * The #108 item-5 misses (a)–(d) are therefore closed by construction, not by the scanner: the edge gate in
     `tools/cap-graph-logic-tests/src/lib.rs` is now explicitly a second check ("the type system is the primary guard"),
     with its allowlist rewritten to the `Derivations` method calls and a second test asserting every allowed site's
     body mentions `lock_cap_graph()`/`CapGraph`/`&mut self`. The gate still exempts `test_harness.rs`, as before — but
     that file can no longer reach the map either, since the privacy boundary is in the type, not the scan.
2. `insert_without_cap_graph` is reachable only under `neo-test-missing-cap-lock-negative` — PASS. Three independent
   layers agree:
   * The method itself is `#[cfg(feature = "neo-test-missing-cap-lock-negative")]` (`process.rs:2008-2014`): without
     the feature it does not exist, so no call site can compile against it.
   * Its only call site in the file is inside `install_cap`'s matching
     `#[cfg(feature = "neo-test-missing-cap-lock-negative")]` branch (`:2264-2272`); the non-negative branch takes
     `lock_cap_graph()` and calls `install_minted_locked` instead (`:2274-2279`). A grep for the identifier across
     `process.rs` finds exactly these two occurrences (definition and call).
   * Visibility is `pub(super)`, so even with the feature on, only code in the `process` module could call it — and
     the only caller is that one branch.
   * Feature graph (`kernel/Cargo.toml:583-585`): `neo-test-missing-cap-lock-negative = []`, implied only by
     `neo-test-q8a-concurrent-install-revoke-negative = ["neo-test", "neo-test-missing-cap-lock-negative"]`. Default
     features are `kernel-diagnostic, accel-sim, inference-preemptible` (`:77`) — no `neo-test` anywhere — so the
     negative gate cannot leak into a product build through defaults or implication. The gate's own comment ("the q8a
     negative gate's deliberately broken install") and the allowlist entry ("the q8a negative gate only") match.
3. `CapGraph::retire` holds no `Arc<Process>` that could be the last reference when its `Vec` drops — PASS. `retire`
   (`process.rs:2163-2189`) builds its work list as `let processes: Vec<Arc<Process>> = self.snapshot().to_vec();` —
   clones of Arcs the guard has already pinned. `snapshot()` (`:2125`) extends `self.pinned` with
   `processes_snapshot()` before returning the slice, so every Arc in the local `Vec` has at least one other live
   reference (the guard's `pinned` copy) at every point in `retire`, including when the local `Vec` drops at the end of
   the function under the lock. The comment states exactly this (CP-91). The pinned copies themselves are never dropped
   under the lock: `CapGraph::drop` (`:2065-2091`) releases the lock first, then hands any pin that `Arc::into_inner`
   proves last to `DEFERRED_PROCESS_DROPS`, which the reaper drops outside any `CAP_GRAPH` section
   (`drop_deferred_processes`). The same clone-of-pinned pattern is used by the revoke walk (`:2489` area, with the same
   comment). `retire`'s other local, `ups`, holds only `CapId` values (Copy), not Arcs. Concrete check that would fail
   if the pattern were broken: a process whose registry reference is dropped by the reaper mid-`retire` still has the
   pinned copy, so the local `Vec`'s drop decrements 2→1, never 1→0 — no `Pmap::drop` (and its TLB shootdown) can run
   under `CAP_GRAPH` from this path.
4. The ADR-010 text now matches `retire_one` and `teardown` — PASS. The #108 item-2/item-6 wording note is fixed
   verbatim: CP-88 now reads "its children are re-parented to its nearest ancestor that is not also being removed, in
   the same `CAP_GRAPH` section, before any other operation can observe the removal. (The code removes the closing
   cap's own edge first, then re-parents, all under the lock; wording corrected 2026-10-29 after Muse's review of
   #166.)" That parenthetical is exactly what `retire_one` (`process.rs:2191-2199`) does —
   `proc.derivations.remove(self, cap_id)` first, then `self.retire(&dying)` re-parents — and `retire_one` is only ever
   called with a held guard (`close_handle` at `:2243` inside `lock_cap_graph()`; test-only `retire_edge_for_test` at
   `:2033`, which takes the lock itself), so "no other operation can observe" holds. `teardown` (`:2203-2214`) matches
   the ADR's "close or teardown" phrasing: it drains the handles, takes every edge (`derivations.take`), calls
   `retire(&dying)` to re-parent survivors, and returns the drained handles for the caller to drop after releasing the
   lock — and both call sites (`:3420`, `:4359`) hold `lock_cap_graph()`. The K1b paragraph ("reachable only through
   `process::edges::Derivations`, whose every read and write takes the caller's `CapGraph` guard") holds for every
   product path. Minor observation, not a failure: the sentence is slightly overbroad as written, since the two
   test-only exceptions — the negative-gate write (item 2) and the `neo-test` read getters (item 1) — do not take a
   guard; both are cfg-gated out of every product build, so the product-path claim is exact.

Overall verdict: PASS. All 4 items pass. The #108 follow-up suggestion (make `derivations` module-private with
CapGraph-only accessors) is implemented as K1b and closes the edge gate's named misses by construction; the negative
gate is confined to its feature by cfg, visibility, and a single call site; `retire`'s Arc handling cannot drop a last
reference under the lock; and the ADR-010 wording now describes the code's actual order, including the parenthetical
the #108 review asked for.

Limitations: `kernel/src/test_harness.rs` (~1.5 MB) could not be read through the MCP tool and the private repo's raw
download 404s without a token, so no scenario leg in it was re-verified; the verdicts above rest on the readable code
and the host KAT crate. The product-image feature list in `tools/xtask/src/main.rs` was not re-fetched at this SHA;
the item-2 verdict rests on the feature graph in `kernel/Cargo.toml` (the negative feature is `[]`, implied only by
another `neo-test-*` feature, and absent from defaults), which is sufficient for reachability.
