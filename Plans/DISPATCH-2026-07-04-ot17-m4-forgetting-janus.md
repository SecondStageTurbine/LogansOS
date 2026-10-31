<!-- SPDX-License-Identifier: CC-BY-4.0 -->
# DISPATCH: OT-17 M4 — forgetting operations (block overwrite + total wipe) — the memory arc's final slice → Janus

**Date:** 2026-07-04
**From:** PM (Claude)
**To:** Janus (main-tier coder). **Branch:** `codex/ot17-m4-forgetting-janus` off `origin/main` — **cut the branch ONLY after the PM confirms the M3 merge is on `origin/main`** (the M3+IPC staging gate is in flight as this is written; M4 builds directly on M3's `memory_logic.rs` / `crypto_erase` / `revoke_delegated_category`). The PM's merge confirmation names the commit; `git fetch origin` and verify your base contains `memory_logic.rs` before starting.
**Status:** DISPATCHED — **the last slice of the ratified OT-17 memory design** (`THINK-2026-06-18-arthur-memory-substrate-design.md`, D1–D6 ratified; slice plan §M4). M1 (store) + M2 (encrypt) + M3 (delegate) are landed; M4 (forget) completes the arc and lets the PM close OT-17 in the ledger.

## 0. What M4 is (and what it is NOT — most of "forget" already exists)
The THINK's slice M4: *"Forgetting operations — per-category key burn (primary) + best-effort block overwrite (secondary). Total wipe = burn all keys. Kill-switch: forgetting unavailable → memories persist until manual deletion. Requires M2."*

Already landed — do NOT rebuild:
- **Per-category key burn** (M2 burn-via-`None`; NIST 800-88 crypto-erase, D5).
- **`forget <category>`** user path incl. revoke-of-delegated-caps-on-burn (M3's `crypto_erase` → `revoke_delegated_category`).
- Truncate-backed whole-file rewrite (M3, on Janus's own NeoFS truncate).

**M4 adds the three missing pieces:**
1. **ISC-1 — Best-effort block overwrite (the 800-88 secondary measure).** After a category's key burn, overwrite the category's `.mem` ciphertext extent with a fixed pattern (zeros) *before* truncate/delete, so the ciphertext doesn't linger on disk after the key is gone. **The honest-payoff discipline applies (the ADR-065 precedent):** whether an in-place NeoFS write actually lands on the *same physical blocks* is an FS-allocation question — you built the truncate path, so decide and **state honestly in the ADR what is proven vs best-effort** (if NeoFS reallocates on write, say so; the key burn remains the *primary* erase and the overwrite is defense-in-depth, never the load-bearing claim).
2. **ISC-2 — Total wipe.** `forget everything` = burn **all** category keys + overwrite + revoke all delegated caps + remove the files — one operation, ordered so a crash mid-wipe still leaves every category key-burned first (key burn is atomic-est and load-bearing; do it for all categories before any overwrite/delete). Post-wipe state ≡ D6 fail-safe (fresh/ephemeral memory).
3. **ISC-3 — The Arthur-facing surface.** Wire both into Arthur's NL/action path consistent with how M3 wired `delegate`/`forget <category>` (mutating, so it graduates per OT-19/OT-26 D2 — confirm; it is irreversible-by-design, which is the point).

## ISCs (gates)
- **ISC-4 — Falsifiable overwrite oracle.** A QEMU scenario (or extension of the M3 scenario family) proving: after `forget <category>`, (a) reads via any remaining handle fail; (b) a raw re-read of the file's former extent (or the FS-level read of the object before delete) shows the overwrite pattern, NOT ciphertext — a hand-checkable byte assertion, not a round-trip; (c) sibling categories are untouched (the M3 sibling discipline); (d) total wipe leaves Arthur booting clean into D6 ephemeral-fresh state.
- **ISC-5 — Kill-switch.** `arthur-memory-forgetting` (or fold into the M-feature ladder as M3 did) **off ⇒ bit-for-bit M3 behavior** — the THINK's stated fallback: memories persist until manual deletion. Regression proves off ≡ M3.
- **ISC-6 — No new invariant (Rule 2), no new lock; nothing in an ISR path.** Userland + NeoFS ops only; if you need any kernel-side FS primitive beyond what exists (as M3 needed truncate), flag it to the PM *before* building it — do not silently grow the kernel surface.
- **ISC-7 — Ledger close.** Update `docs/OPEN-THREADS.md` OT-17 → **DONE (M1–M4 landed)**, carrying the residual note (snapshot-rollback banked to TPM/off-disk-key hardening). Short ADR for M4 (number per the current sequence), including the ISC-1 proven-vs-best-effort statement.

## Scope guard
No changes to M3's delegation logic or the cap-check core (both just AAG-verified — touch only what M4 adds). No new QEMU scenario *file* if extending the M3 scenario is cleaner. One slice (OT-22): forgetting only.

## Gate + close
- **Kernel-touching (test harness/scenario at minimum) ⇒ the 4-shard QEMU `workflow_dispatch` gate is mandatory.** Runner note: two gates are in flight as this is written (the M3+IPC staging gate + Oracle's OT-34 fix gate) — yours will naturally land after they drain; if the runners are still contended when you're ready, tell the PL and stagger.
- **Close-gate `scripts/worker-close-gate.sh`** (rebase-freshness, full host set, SPDX, self-stat). RETURN: green push-CI URL + green QEMU `workflow_dispatch` URL + your `git diff --stat` vs `main` + the ISC-1 honest-payoff statement (proven vs best-effort overwrite).
- **Disk reclaim** after the PM confirms merge.

---
## Relay — what to tell Janus
- **Read:** `git fetch origin && git show origin/main:Plans/DISPATCH-2026-07-04-ot17-m4-forgetting-janus.md` (+ the memory THINK `Plans/THINK-2026-06-18-arthur-memory-substrate-design.md` §slices, ADR-065). **Branch:** `codex/ot17-m4-forgetting-janus` off `origin/main` — **wait for the PM's M3-merge confirmation first** (verify `memory_logic.rs` is in your base).
- **One-line:** **Build OT-17 M4, the memory arc's final slice: forgetting operations.** Key burn + `forget <category>` + delegated-cap revoke already landed in M2/M3 — don't rebuild them. M4 adds: (1) best-effort **block overwrite** of the ciphertext extent after key burn (800-88 secondary; state honestly in the ADR whether NeoFS in-place writes hit the same blocks — proven vs best-effort, key burn stays the primary erase); (2) **total wipe** (`forget everything` — burn ALL keys first, then overwrite/delete, crash-ordered so keys always burn before anything else; post-state ≡ D6 ephemeral-fresh); (3) the Arthur NL surface (mutating ⇒ confirm). Falsifiable overwrite oracle (raw re-read shows the pattern, not ciphertext; siblings untouched; total wipe boots clean). Kill-switch: forgetting-off ≡ M3 bit-for-bit. No new invariant/lock; if you need a new kernel FS primitive, flag it before building. Update OT-17 → DONE + a short M4 ADR. **Kernel-touching ⇒ QEMU gate mandatory** (two gates in flight now — stagger if contended). Close-gate; RETURN both green CI URLs + diff-stat + the proven-vs-best-effort statement.
